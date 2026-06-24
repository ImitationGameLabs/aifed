//! aifed-daemon - Background daemon for aifed
//!
//! This daemon provides LSP services for a single workspace via HTTP over a TCP
//! loopback port. Each daemon instance is bound to exactly one workspace; the
//! CLI discovers it via a per-workspace endpoint file (port + bearer token).

mod args;
mod error;
mod history;
mod idle;
mod languages;
mod lsp;
mod server;

use aifed_common::{
    DaemonEndpoint, endpoint_path, ensure_default_config, load_registry_for_workspace, lock_path,
    log_path, read_endpoint, write_endpoint,
};
use anyhow::Context;
use args::Args;
use clap::Parser;
use history::HistoryManager;
use idle::IdleMonitor;
use languages::ConfiguredLanguageServerConfig;
use lsp::LanguageServerManager;
use server::{DaemonState, build_router};
use std::fs::{File, TryLockError};
use std::io;
use std::path::Path;
use std::sync::{Arc, RwLock};
use tokio::net::TcpListener;
use tracing::Level;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// Probe whether a live, authentic daemon is already serving `workspace`.
///
/// Reads the workspace's endpoint file and, if present, performs a token-bearing
/// `/health` probe over TCP. Returns `true` only if a daemon answers with HTTP
/// 200 *and* a body containing our `ApiResponse` envelope (`"success":true`) —
/// so a stale endpoint, a port reused by an unrelated process, or a different
/// aifed daemon (wrong token → 401) all correctly report as "not running".
fn check_existing_daemon(endpoint_file: &Path) -> bool {
    let endpoint = match read_endpoint(endpoint_file) {
        Ok(e) => e,
        Err(_) => return false,
    };
    probe_health(endpoint.port, &endpoint.token)
}

/// Raw TCP `/health` probe (kept dependency-free — no `aifed-daemon-client`).
fn probe_health(port: u16, token: &str) -> bool {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let Ok(addr) = format!("127.0.0.1:{port}").parse() else {
        return false;
    };
    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(500)) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let request = format!(
        "GET /api/v1/health HTTP/1.0\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut response = String::new();
    if stream.read_to_string(&mut response).is_err() {
        return false;
    }
    // Anchor the status line (avoid matching a ` 200 ` substring in a header),
    // and require our `ApiResponse` success envelope in the body — the token
    // already rejects other aifed daemons (401); this body check rejects an
    // unrelated process that happens to occupy the port and return some 200.
    let status_ok = response.starts_with("HTTP/1.0 200") || response.starts_with("HTTP/1.1 200");
    status_ok && response.contains("\"success\":true")
}

/// Prefix marking daemon bearer tokens so they are recognizable as secrets.
const TOKEN_PREFIX: &str = "sk-";

/// Generate a random 256-bit bearer token, `sk-`-prefixed and hex-encoded.
fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("getrandom failed");
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{TOKEN_PREFIX}{hex}")
}

/// SHA-256 digest of `input` as a fixed 32-byte array.
fn sha256(input: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    sha2::Sha256::digest(input).into()
}

/// Acquire an exclusive, non-blocking advisory lock on the lock file via std
/// `File::try_lock`. The returned File handle must be kept open for the lock to
/// remain held; dropping it (or process exit) releases the lock.
fn acquire_lock(lock_path: &std::path::Path) -> anyhow::Result<File> {
    // Ensure parent directory exists
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create lock directory: {}", parent.display()))?;
    }

    // Create/open the lock file. It is an empty sentinel we never write, so open
    // read+write+create without truncate. std `try_lock` works on any open mode.
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .with_context(|| format!("Failed to create lock file: {}", lock_path.display()))?;

    // Acquire an exclusive, non-blocking advisory lock. `WouldBlock` means the
    // lock is already held (another daemon owns it); any other error is real.
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => anyhow::bail!(
            "Another daemon is starting for this workspace (lock file: {})",
            lock_path.display()
        ),
        Err(TryLockError::Error(e)) => {
            Err(e).with_context(|| format!("Failed to acquire lock file: {}", lock_path.display()))
        }
    }
}

/// Initialize logging with optional file output.
///
/// When `log_stderr` is true, only outputs to stderr (ignores `log_file`).
/// When `log_file` is provided, only outputs to file with size-based rotation.
/// Otherwise, only outputs to stderr.
fn init_logging(
    level: &str,
    log_file: Option<&std::path::Path>,
    log_stderr: bool,
) -> anyhow::Result<()> {
    use logroller::{LogRoller, LogRollerBuilder, Rotation, RotationSize};
    use time::macros::format_description;
    use tracing_subscriber::fmt::time::OffsetTime;

    // Parse log level
    let level: Level = level
        .parse()
        .with_context(|| format!("Invalid log level: {}", level))?;

    // Build env filter
    let env_filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env()
        .unwrap_or_else(|_| EnvFilter::new(level.to_string()));

    // Local timezone timer for log timestamps
    let local_offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let timer = OffsetTime::new(
        local_offset,
        format_description!("[year]-[month]-[day] [hour]:[minute]:[second]"),
    );

    if log_stderr {
        // Only stderr output (--log-stderr takes priority)
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().with_timer(timer))
            .init();
    } else if let Some(log_path) = log_file {
        // File output with rotation
        let parent = log_path
            .parent()
            .context("Log file has no parent directory")?;
        let filename = log_path
            .file_name()
            .context("Log file has no filename")?
            .to_string_lossy();
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create log directory: {}", parent.display()))?;

        let appender = LogRollerBuilder::new(parent, Path::new(filename.as_ref()))
            .rotation(Rotation::SizeBased(RotationSize::MB(5)))
            .max_keep_files(2)
            .build()
            .with_context(|| "Failed to create log roller")?;

        // Wrap LogRoller in Arc<Mutex> for thread-safe sharing
        let appender = Arc::new(std::sync::Mutex::new(appender));

        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_timer(timer)
                    .with_writer(move || {
                        struct LogWriter(Arc<std::sync::Mutex<LogRoller>>);
                        impl io::Write for LogWriter {
                            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                                self.0.lock().unwrap().write(buf)
                            }
                            fn flush(&mut self) -> io::Result<()> {
                                self.0.lock().unwrap().flush()
                            }
                        }
                        Box::new(LogWriter(appender.clone()))
                    }),
            )
            .init();
    } else {
        // No log file configured, stderr only
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().with_timer(timer))
            .init();
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse CLI arguments
    let args = Args::parse();

    // Fatal: daemon cannot function without a config file (no LSP servers to serve).
    // The CLI treats this as non-fatal because basic file ops (read/edit) still work.
    ensure_default_config().context("Failed to initialize default config")?;

    // Canonicalize workspace path
    let workspace = args.workspace.canonicalize().map_err(|e| {
        anyhow::anyhow!(
            "Failed to canonicalize workspace path '{}': {}",
            args.workspace.display(),
            e
        )
    })?;

    // Determine paths (all derived from the workspace, no per-invocation override)
    let endpoint_file = endpoint_path(&workspace).expect("Failed to generate endpoint path");
    let lock = lock_path(&workspace).expect("Failed to generate lock path");
    let default_log = log_path(&workspace).expect("Failed to generate log path");
    let log_file = args.log_file.as_deref().unwrap_or(&default_log);

    // Initialize logging (before duplicate check, so we can log errors)
    init_logging(
        &args.log_level,
        if args.log_stderr { None } else { Some(log_file) },
        args.log_stderr,
    )?;

    // Check for existing daemon
    if check_existing_daemon(&endpoint_file) {
        anyhow::bail!(
            "Daemon already running for workspace: {}",
            workspace.display()
        );
    }

    // Acquire lock to prevent race condition.
    // Held for the daemon's entire lifetime; released on drop (i.e. process
    // exit), NOT by the remove_file cleanup near the end of main.
    let _lock_file = acquire_lock(&lock)?;

    tracing::info!(
        "Starting aifed-daemon for workspace: {}",
        workspace.display()
    );
    tracing::info!("Endpoint file: {}", endpoint_file.display());
    tracing::info!("Lock file: {}", lock.display());
    if !args.log_stderr {
        tracing::info!("Log file: {}", log_file.display());
    }

    let registry = load_registry_for_workspace(Some(&workspace)).with_context(|| {
        format!(
            "Failed to load LSP config for workspace: {}",
            workspace.display()
        )
    })?;

    // Initialize LSP manager
    let lsp_manager = Arc::new(LanguageServerManager::new());

    // Register language server configs
    for config in registry.entries().iter().cloned() {
        lsp_manager
            .register_config(ConfiguredLanguageServerConfig::new(config))
            .await;
    }
    tracing::debug!(
        "Registered language server configs: {:?}",
        registry
            .entries()
            .iter()
            .map(|entry| format!("{} ({})", entry.language, entry.display_name()))
            .collect::<Vec<_>>()
    );

    // Auto-detect languages and start LSP servers
    let detected = registry.detect_languages_for_workspace(&workspace);
    tracing::info!("Detected languages: {:?}", detected);

    for entry in &detected {
        match lsp_manager.start(&entry.language, workspace.clone()).await {
            Ok(()) => tracing::info!("Started LSP server for: {}", entry.language),
            Err(e) => tracing::error!("Failed to start LSP server for {}: {}", entry.language, e),
        }
    }

    // Initialize idle monitor
    let idle_monitor = Arc::new(IdleMonitor::new(args.idle_timeout_secs));
    let monitor_clone = idle_monitor.clone();
    monitor_clone.start_monitor();

    // Initialize history manager
    let history_manager = Arc::new(HistoryManager::new());

    // Bind the loopback port first so we know the actual port to advertise.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .with_context(|| "Failed to bind loopback TCP port")?;
    let port = listener
        .local_addr()
        .with_context(|| "Failed to read bound port")?
        .port();
    let address = format!("127.0.0.1:{port}");

    // Generate the bearer token and publish the endpoint file so the CLI can
    // discover us. The lock is already held and the port is bound, so this is
    // the authoritative contact info for this daemon.
    let token = generate_token();
    let endpoint = DaemonEndpoint {
        workspace: workspace.to_string_lossy().into_owned(),
        pid: std::process::id(),
        port,
        token: token.clone(),
    };
    write_endpoint(&endpoint_file, &endpoint)
        .with_context(|| format!("Failed to write endpoint file: {}", endpoint_file.display()))?;
    tracing::info!("Listening on {}", address);

    // Keep only the token's SHA-256 in memory; the plaintext lives solely in
    // the endpoint file (for the CLI). The middleware compares hashes, so a
    // process memory dump yields no usable credential.
    let token_hash = sha256(token.as_bytes());

    // Create shared state
    let state = DaemonState {
        workspace: workspace.clone(),
        lsp_manager,
        history_manager,
        idle_monitor: idle_monitor.clone(),
        clipboard: Arc::new(RwLock::new(None)),
        address,
        token_hash,
        log_path: log_file.to_path_buf(),
    };

    // Build router
    let app = build_router(state);

    // Setup graceful shutdown
    let idle_for_shutdown = idle_monitor.clone();
    let shutdown_signal = async move {
        let mut shutdown_rx = idle_for_shutdown.subscribe_shutdown();
        tokio::select! {
            _ = shutdown_rx.recv() => {
                tracing::info!("Idle timeout shutdown triggered");
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Ctrl+C received, shutting down");
            }
        }
    };

    // Run server
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await?;

    // Cleanup
    tracing::info!("Shutting down daemon");
    // Delete the endpoint file only if it still describes THIS daemon, so we
    // never clobber a newer daemon that may have already overwritten it.
    if let Ok(current) = read_endpoint(&endpoint_file)
        && current.pid == endpoint.pid
        && current.token == endpoint.token
    {
        let _ = std::fs::remove_file(&endpoint_file);
    }
    if let Err(e) = std::fs::remove_file(&lock) {
        tracing::warn!("Failed to remove lock file: {}", e);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each `acquire_lock` does its own `open()`, so two calls contend on the
    // same lock — do not refactor to share a File handle (e.g. `try_clone()`),
    // which would share one open file description and mask contention.

    #[test]
    fn acquire_lock_then_second_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.lock");

        let _first = acquire_lock(&path).expect("first lock should succeed");
        let second = acquire_lock(&path);
        assert!(
            second.is_err(),
            "a second concurrent lock on the same file must be rejected"
        );
        // `_first` dropped here, releasing the lock.
    }

    #[test]
    fn lock_released_on_drop() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.lock");

        {
            let _first = acquire_lock(&path).expect("first lock should succeed");
        }

        let _again = acquire_lock(&path)
            .expect("lock must be reacquirable after the previous handle is dropped");
    }
}
