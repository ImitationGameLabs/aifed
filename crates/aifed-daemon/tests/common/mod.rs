//! Test utilities for socket-based integration tests

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::{Method, Request, Uri as HyperUri};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use hyperlocal::{UnixConnector, Uri};
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use std::{fs, thread};

// --- HTTP Client for Testing ---

/// HTTP client that connects via Unix socket (for testing)
pub struct HttpClient {
    client: Client<UnixConnector, Full<Bytes>>,
    socket_path: std::path::PathBuf,
}

impl HttpClient {
    pub fn new(socket_path: &Path) -> Self {
        let client = Client::builder(TokioExecutor::new()).build(UnixConnector);
        Self { client, socket_path: socket_path.to_path_buf() }
    }

    fn uri(&self, path: &str) -> HyperUri {
        Uri::new(&self.socket_path, path).into()
    }

    pub async fn get(&self, path: &str) -> Result<Response, Box<dyn std::error::Error>> {
        let uri = self.uri(path);
        let req =
            Request::builder().method(Method::GET).uri(uri).body(Full::new(Bytes::new())).unwrap();

        let resp = self.client.request(req).await?;
        Ok(Response::from_hyper(resp).await)
    }

    pub async fn post(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<Response, Box<dyn std::error::Error>> {
        let uri = self.uri(path);
        let json = serde_json::to_string(body).unwrap();
        let req = Request::builder()
            .method(Method::POST)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Full::new(Bytes::from(json)))
            .unwrap();

        let resp = self.client.request(req).await?;
        Ok(Response::from_hyper(resp).await)
    }
}

// --- Response ---

pub struct Response {
    pub status: hyper::StatusCode,
    body: String,
}

impl Response {
    async fn from_hyper(resp: hyper::Response<hyper::body::Incoming>) -> Self {
        let status = resp.status();
        let body = resp.collect().await.unwrap().to_bytes();
        Self { status, body: String::from_utf8_lossy(&body).to_string() }
    }

    pub fn json<T: DeserializeOwned>(&self) -> T {
        serde_json::from_str(&self.body).unwrap()
    }

    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }
}

// --- Daemon Fixture ---

/// Test fixture that spawns a daemon process with a Rust workspace
#[allow(dead_code)]
pub struct DaemonFixture {
    /// Holds TempDir to prevent cleanup until struct is dropped
    _workspace: tempfile::TempDir,
    socket_path: std::path::PathBuf,
    daemon: Child,
    /// HTTP client for making requests to the daemon
    pub client: HttpClient,
    /// Path to the main.rs file in the test workspace
    pub main_rs_path: String,
}

impl DaemonFixture {
    /// Create a new test fixture with a unique socket path
    pub async fn new() -> Self {
        Self::new_with_prefix("test").await
    }

    /// Create with a custom prefix for socket path (for different test files)
    pub async fn new_with_prefix(prefix: &str) -> Self {
        // Create test workspace with a Rust file
        let workspace = tempfile::tempdir().unwrap();
        fs::write(
            workspace.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::create_dir_all(workspace.path().join("src")).unwrap();

        let main_rs_content = r#"fn main() {
    let greeting = "hello";
    println!("{}", greeting);
}
"#;
        fs::write(workspace.path().join("src/main.rs"), main_rs_content).unwrap();

        let main_rs_path = workspace.path().join("src/main.rs").to_string_lossy().to_string();

        // Generate unique socket path
        static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);
        let test_id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);

        let socket_path = dirs::cache_dir().unwrap().join("aifed").join(format!(
            "{}-{}-{}.sock",
            prefix,
            std::process::id(),
            test_id
        ));

        // Ensure clean state
        let _ = fs::remove_file(&socket_path);
        if let Some(parent) = socket_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }

        // Build daemon binary
        let status = Command::new("cargo").args(["build", "-p", "aifed-daemon"]).status().unwrap();
        assert!(status.success(), "Failed to build aifed-daemon");

        // Spawn daemon process (use workspace root target directory)
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_dir =
            manifest_dir.parent().and_then(|p| p.parent()).expect("Failed to find workspace root");
        let daemon_path = workspace_dir.join("target/debug/aifed-daemon");
        let daemon = Command::new(&daemon_path)
            .arg("--workspace")
            .arg(workspace.path())
            .arg("--socket")
            .arg(&socket_path)
            .arg("--idle-timeout-secs")
            .arg("3600")
            .spawn()
            .unwrap();

        // Wait for socket to be ready
        let client = HttpClient::new(&socket_path);
        Self::wait_for_socket(&client).await;

        Self { _workspace: workspace, socket_path, daemon, client, main_rs_path }
    }

    async fn wait_for_socket(client: &HttpClient) {
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(10) {
            if let Ok(resp) = client.get("/api/v1/health").await
                && resp.is_success()
            {
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }
        panic!("Daemon did not start within 10 seconds");
    }
}

impl Drop for DaemonFixture {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
        let _ = fs::remove_file(&self.socket_path);
    }
}

// --- Re-export types for tests ---
