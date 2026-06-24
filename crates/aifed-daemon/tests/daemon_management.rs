//! Daemon management integration tests.
//!
//! Exercises the daemon lifecycle and management API via the typed
//! `DaemonClient` (health, status, server list, start/stop).

mod common;

use aifed_common::read_endpoint;
use aifed_daemon_client::DaemonClient;
use common::DaemonFixture;

#[tokio::test]
async fn test_health_endpoint() {
    let fixture = DaemonFixture::new().await;
    let health = fixture.client.health().await.unwrap();
    assert_eq!(health.status, "ok");
}

#[tokio::test]
async fn test_wrong_token_is_rejected() {
    let fixture = DaemonFixture::new().await;

    // A client presenting the wrong bearer token must be rejected (401 → Err),
    // while the fixture's authentic client still succeeds.
    let port = read_endpoint(fixture.endpoint_file()).unwrap().port;
    let imposter = DaemonClient::new(format!("http://127.0.0.1:{port}"), "not-the-token");
    assert!(
        imposter.health().await.is_err(),
        "wrong token must be rejected"
    );
    assert!(
        fixture.client.health().await.is_ok(),
        "correct token must work"
    );
}

#[tokio::test]
async fn test_discover_returns_none_without_daemon() {
    // A workspace with no running daemon has no endpoint file (or a stale one);
    // discover must report None so the CLI spawns fresh rather than connecting
    // to nothing.
    let dir = tempfile::tempdir().unwrap();
    assert!(DaemonClient::discover(dir.path()).await.is_none());
}

#[tokio::test]
async fn test_status_endpoint() {
    let fixture = DaemonFixture::new().await;
    let status = fixture.client.status().await.unwrap();
    assert!(
        !status.workspace.is_empty(),
        "workspace path should not be empty"
    );
}

#[tokio::test]
async fn test_list_servers() {
    let fixture = DaemonFixture::new().await;
    let servers = fixture.client.list_servers().await.unwrap();
    // The daemon auto-detects rust and starts rust-analyzer.
    assert!(!servers.servers.is_empty());
}

#[tokio::test]
async fn test_start_and_stop_server() {
    let fixture = DaemonFixture::new().await;

    // Stop the auto-started rust server, then start it again.
    fixture.client.stop_server("rust", false).await.unwrap();
    fixture.client.start_server("rust").await.unwrap();
}
