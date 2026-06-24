//! Test utilities for TCP-based integration tests.
//!
//! Each fixture gets its own tempdir workspace, which yields a unique endpoint
//! file path (derived from the workspace), so parallel tests never collide.

use aifed_common::{endpoint_path, lock_path};
use aifed_daemon_client::DaemonClient;
use std::process::{Child, Command};
use std::time::Duration;
use std::{fs, thread};

/// Test fixture that spawns a daemon process with a Rust workspace.
#[allow(dead_code)]
pub struct DaemonFixture {
    /// Holds TempDir to prevent cleanup until the struct is dropped.
    _workspace: tempfile::TempDir,
    /// Isolates the daemon from the user's real `~/.config/aifed/config.toml`
    /// so tests are hermetic — the daemon writes/reads only this temp dir.
    _config_dir: tempfile::TempDir,
    /// Endpoint + lock files, removed in Drop (the daemon is killed, not shut
    /// down gracefully, so it cannot clean them up itself).
    endpoint_file: std::path::PathBuf,
    lock_file: std::path::PathBuf,
    daemon: Child,
    /// Client for making requests to the daemon.
    pub client: DaemonClient,
    /// Path to the main.rs file in the test workspace.
    pub main_rs_path: String,
}

impl DaemonFixture {
    /// Spawn a daemon for a fresh Rust workspace and wait until it answers.
    pub async fn new() -> Self {
        // Create test workspace with a Rust file.
        let workspace = tempfile::tempdir().unwrap();
        fs::write(
            workspace.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("aifed.toml"),
            r#"[[lsp]]
language = "rust"
root_markers = ["Cargo.toml"]
command = "rust-analyzer"
display_name = "rust-analyzer"
"#,
        )
        .unwrap();
        fs::create_dir_all(workspace.path().join("src")).unwrap();
        let main_rs_content = r#"fn main() {
    let greeting = "hello";
    println!("{}", greeting);
}
"#;
        fs::write(workspace.path().join("src/main.rs"), main_rs_content).unwrap();
        let main_rs_path = workspace
            .path()
            .join("src/main.rs")
            .to_string_lossy()
            .to_string();

        // Per-workspace endpoint + lock paths (unique per tempdir).
        let endpoint_file = endpoint_path(workspace.path()).unwrap();
        let lock_file = lock_path(workspace.path()).unwrap();

        // Clean slate in case a prior crashed run left files behind.
        let _ = fs::remove_file(&endpoint_file);
        let _ = fs::remove_file(&lock_file);

        // Build daemon binary.
        let status = Command::new("cargo")
            .args(["build", "-p", "aifed-daemon"])
            .status()
            .unwrap();
        assert!(status.success(), "Failed to build aifed-daemon");

        // Hermetic global config dir.
        let config_dir = tempfile::tempdir().unwrap();

        // Spawn daemon (endpoint path is derived from the workspace, so no
        // transport path argument is needed).
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_dir = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("Failed to find workspace root");
        let daemon_path = workspace_dir.join("target/debug/aifed-daemon");
        let daemon = Command::new(&daemon_path)
            .arg("--workspace")
            .arg(workspace.path())
            .arg("--idle-timeout-secs")
            .arg("3600")
            .env("AIFED_CONFIG_DIR", config_dir.path())
            .spawn()
            .unwrap();

        // Wait for the daemon to publish its endpoint and answer /health.
        let client = wait_for_daemon(workspace.path()).await;

        Self {
            _workspace: workspace,
            _config_dir: config_dir,
            endpoint_file,
            lock_file,
            daemon,
            client,
            main_rs_path,
        }
    }
}

/// Poll discovery until the daemon publishes its endpoint and answers.
async fn wait_for_daemon(workspace_root: &std::path::Path) -> DaemonClient {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if let Some(client) = DaemonClient::discover(workspace_root).await {
            return client;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("Daemon did not start within 10 seconds");
}

impl DaemonFixture {
    /// Path to the daemon's endpoint file (for tests that need the port/token).
    #[allow(dead_code)]
    pub fn endpoint_file(&self) -> &std::path::Path {
        &self.endpoint_file
    }
}

impl Drop for DaemonFixture {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
        let _ = fs::remove_file(&self.endpoint_file);
        let _ = fs::remove_file(&self.lock_file);
    }
}
