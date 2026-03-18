//! Daemon management integration tests
//!
//! Tests for daemon lifecycle and HTTP API endpoints:
//! - Health check: GET /api/v1/health
//! - Status: GET /api/v1/status
//! - Server list: GET /api/v1/lsp/servers
//! - Start server: POST /api/v1/lsp/servers/start
//! - Stop server: POST /api/v1/lsp/servers/stop

mod common;

use aifed_common::{
    ApiResponse, HealthResponse, ServersResponse, StartServerRequest, StatusResponse,
    StopServerRequest,
};
use common::DaemonFixture;

#[tokio::test]
async fn test_health_endpoint() {
    let fixture = DaemonFixture::new().await;
    let resp = fixture.client.get("/api/v1/health").await.unwrap();

    assert!(resp.is_success());
    let json: ApiResponse<HealthResponse> = resp.json();
    assert!(json.success);
    assert_eq!(json.data.unwrap().status, "ok");
}

#[tokio::test]
async fn test_status_endpoint() {
    let fixture = DaemonFixture::new().await;
    let resp = fixture.client.get("/api/v1/status").await.unwrap();

    assert!(resp.is_success());
    let json: ApiResponse<StatusResponse> = resp.json();
    assert!(json.success);
    let data = json.data.unwrap();
    assert!(!data.workspace.is_empty(), "Workspace path should not be empty");
}

#[tokio::test]
async fn test_list_servers() {
    let fixture = DaemonFixture::new().await;
    let resp = fixture.client.get("/api/v1/lsp/servers").await.unwrap();

    assert!(resp.is_success());
    let json: ApiResponse<ServersResponse> = resp.json();
    assert!(json.success);
    // Should have auto-detected rust and started rust-analyzer
    assert!(!json.data.unwrap().servers.is_empty());
}

#[tokio::test]
async fn test_start_and_stop_server() {
    let fixture = DaemonFixture::new().await;

    // Stop the auto-started rust server
    let resp = fixture
        .client
        .post(
            "/api/v1/lsp/servers/stop",
            &StopServerRequest { language: "rust".into(), force: false },
        )
        .await
        .unwrap();
    assert!(resp.is_success());

    // Start it again
    let resp = fixture
        .client
        .post("/api/v1/lsp/servers/start", &StartServerRequest { language: "rust".into() })
        .await
        .unwrap();
    assert!(resp.is_success());
}
