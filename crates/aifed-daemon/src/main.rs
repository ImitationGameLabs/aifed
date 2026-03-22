//! aifed-daemon - Background daemon for aifed
//!
//! This daemon provides LSP services for a single workspace via HTTP over Unix socket.
//! Each daemon instance is bound to exactly one workspace.

mod args;
mod detection;
mod error;
mod history;
mod idle;
mod languages;
mod lsp;
mod server;

use aifed_common::{lock_path, log_path, socket_path};
use anyhow::Context;
use args::Args;
use clap::Parser;
use detection::detect;
use history::HistoryManager;
use idle::IdleMonitor;
use languages::RustAnalyzerConfig;
use lsp::LanguageServerManager;
use server::{DaemonState, build_router};
use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::sync::Arc;
use tokio::net::UnixListener;
use tracing::Level;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// Try to connect to an existing daemon via socket.
/// Returns Ok(()) if daemon is already running (connection succeeded).
/// Returns Err if no daemon is running or connection failed.
fn check_existing_daemon(socket: &std::path::Path) -> anyhow::Result<()> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    // Try to connect to the socket
    let mut stream = match UnixStream::connect(socket) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            // Socket exists but no one is listening - stale socket
            return Err(anyhow::anyhow!("Stale socket file exists"));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Socket doesn't exist - no daemon running
            return Err(anyhow::anyhow!("No daemon running"));
        }
        Err(e) => {
            return Err(anyhow::anyhow!("Failed to connect to socket: {}", e));
        }
    };

    // Send a simple HTTP request to verify it's actually our daemon (not some other process)
    // We ignore errors here since we just want to confirm the daemon exists
    let _ = stream.write_all(b"GET /health HTTP/1.0\r\n\r\n");
    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);

    // If we got here, daemon is running
    Ok(())
}

/// Acquire an exclusive lock on the lock file using flock.
/// Returns the File handle that must be kept open for the lock to remain held.
///
/// TODO: This uses Unix-specific `flock` via libc. For cross-platform support,
/// consider using `fs4` or `fd-lock` crates which provide platform-agnostic file locking.
fn acquire_lock(lock_path: &std::path::Path) -> anyhow::Result<File> {
    // Ensure parent directory exists
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create lock directory: {}", parent.display()))?;
    }

    // Create/open the lock file
    let file = File::create(lock_path)
        .with_context(|| format!("Failed to create lock file: {}", lock_path.display()))?;

    // Try to acquire exclusive lock (non-blocking)
    const LOCK_EX: i32 = 2; // Exclusive lock
    const LOCK_NB: i32 = 4; // Non-blocking
    let result = unsafe { libc::flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) };
    if result != 0 {
        // Lock is held by another process
        anyhow::bail!(
            "Another daemon is starting for this workspace (lock file: {})",
            lock_path.display()
        );
    }

    Ok(file)
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
    let level: Level = level.parse().with_context(|| format!("Invalid log level: {}", level))?;

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
        let parent = log_path.parent().context("Log file has no parent directory")?;
        let filename = log_path.file_name().context("Log file has no filename")?.to_string_lossy();
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
            .with(tracing_subscriber::fmt::layer().with_timer(timer).with_writer(move || {
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
            }))
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

    // Canonicalize workspace path
    let workspace = args.workspace.canonicalize().map_err(|e| {
        anyhow::anyhow!(
            "Failed to canonicalize workspace path '{}': {}",
            args.workspace.display(),
            e
        )
    })?;

    // Determine paths
    let socket = args
        .socket
        .unwrap_or_else(|| socket_path(&workspace).expect("Failed to generate socket path"));
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
    if check_existing_daemon(&socket).is_ok() {
        anyhow::bail!(
            "Daemon already running for workspace: {}\nSocket: {}",
            workspace.display(),
            socket.display()
        );
    }

    // Acquire lock to prevent race condition
    let _lock_file = acquire_lock(&lock)?;

    // Remove stale socket if present
    if socket.exists() {
        std::fs::remove_file(&socket)
            .with_context(|| format!("Failed to remove stale socket: {}", socket.display()))?;
    }

    // Ensure socket directory exists
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create socket directory: {}", parent.display()))?;
    }

    tracing::info!("Starting aifed-daemon for workspace: {}", workspace.display());
    tracing::info!("Socket path: {}", socket.display());
    tracing::info!("Lock file: {}", lock.display());
    if !args.log_stderr {
        tracing::info!("Log file: {}", log_file.display());
    }

    // Initialize LSP manager
    let lsp_manager = Arc::new(LanguageServerManager::new());

    // Register language server configs
    lsp_manager.register_config(RustAnalyzerConfig).await;
    tracing::debug!("Registered language server configs: rust (rust-analyzer)");

    // Auto-detect languages and start LSP servers
    let detected = detect(&workspace);
    tracing::info!("Detected languages: {:?}", detected);

    for lang in &detected {
        match lsp_manager.start(lang, workspace.clone()).await {
            Ok(()) => tracing::info!("Started LSP server for: {}", lang),
            Err(e) => tracing::error!("Failed to start LSP server for {}: {}", lang, e),
        }
    }

    // Initialize idle monitor
    let idle_monitor = Arc::new(IdleMonitor::new(args.idle_timeout_secs));
    let monitor_clone = idle_monitor.clone();
    monitor_clone.start_monitor();

    // Initialize history manager
    // Create shared state
    let history_manager = Arc::new(HistoryManager::new());
    let state = DaemonState {
        workspace: workspace.clone(),
        lsp_manager,
        history_manager,
        idle_monitor: idle_monitor.clone(),
        socket_path: socket.clone(),
        log_path: log_file.to_path_buf(),
    };

    // Build router
    let app = build_router(state);

    // Bind to Unix socket
    let listener = UnixListener::bind(&socket)?;
    tracing::info!("Listening on socket: {}", socket.display());

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
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal).await?;

    // Cleanup
    tracing::info!("Shutting down daemon");
    if let Err(e) = std::fs::remove_file(&socket) {
        tracing::warn!("Failed to remove socket file: {}", e);
    }
    if let Err(e) = std::fs::remove_file(&lock) {
        tracing::warn!("Failed to remove lock file: {}", e);
    }

    Ok(())
}
