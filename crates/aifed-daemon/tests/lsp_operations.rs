//! LSP operations integration tests.
//!
//! Exercises document synchronization and LSP queries via the typed
//! `DaemonClient`. The daemon auto-starts rust-analyzer for Rust workspaces.

mod common;

use aifed_common::{
    ClientError, ContentChange, DiagnosticsRequest, DidChangeRequest, DidCloseRequest,
    DidOpenRequest, HoverRequest, LspPositionRequest, Position, RenameRequest,
};
use common::DaemonFixture;

const MAIN_RS: &str = "fn main() {}\n";

fn open_req(path: &str, text: &str) -> DidOpenRequest {
    DidOpenRequest {
        language: "rust".into(),
        file_path: path.into(),
        language_id: "rust".into(),
        version: 1,
        text: text.into(),
    }
}

// --- Document Synchronization ---

#[tokio::test]
async fn test_did_open_with_server() {
    let fixture = DaemonFixture::new().await;
    fixture
        .client
        .did_open(open_req(&fixture.main_rs_path, MAIN_RS))
        .await
        .unwrap();
}

#[tokio::test]
async fn test_did_change_with_server() {
    let fixture = DaemonFixture::new().await;
    fixture
        .client
        .did_open(open_req(&fixture.main_rs_path, MAIN_RS))
        .await
        .unwrap();

    let change = DidChangeRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        version: 2,
        content_changes: vec![ContentChange {
            range: None, // Full document sync
            text: "fn main() { println!(\"updated\"); }".into(),
        }],
    };
    fixture.client.did_change(change).await.unwrap();
}

#[tokio::test]
async fn test_did_close_with_server() {
    let fixture = DaemonFixture::new().await;
    fixture
        .client
        .did_open(open_req(&fixture.main_rs_path, MAIN_RS))
        .await
        .unwrap();

    let close =
        DidCloseRequest { language: "rust".into(), file_path: fixture.main_rs_path.clone() };
    fixture.client.did_close(close).await.unwrap();
}

#[tokio::test]
async fn test_document_sync_full_workflow() {
    let fixture = DaemonFixture::new().await;
    fixture
        .client
        .did_open(open_req(&fixture.main_rs_path, MAIN_RS))
        .await
        .unwrap();

    for version in 2..=3 {
        let change = DidChangeRequest {
            language: "rust".into(),
            file_path: fixture.main_rs_path.clone(),
            version,
            content_changes: vec![ContentChange {
                range: None,
                text: format!("// v{version}\nfn main() {{}}"),
            }],
        };
        fixture.client.did_change(change).await.unwrap();
    }

    let close =
        DidCloseRequest { language: "rust".into(), file_path: fixture.main_rs_path.clone() };
    fixture.client.did_close(close).await.unwrap();
}

// --- LSP Queries ---

#[tokio::test]
async fn test_hover_with_server() {
    let fixture = DaemonFixture::new().await;
    fixture
        .client
        .did_open(open_req(&fixture.main_rs_path, MAIN_RS))
        .await
        .unwrap();

    let req = HoverRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        position: Position { line: 0, character: 3 },
    };
    // Succeeds (may carry empty hover info).
    let _ = fixture.client.hover(req).await.unwrap();
}

#[tokio::test]
async fn test_definition_with_server() {
    let fixture = DaemonFixture::new().await;
    fixture
        .client
        .did_open(open_req(&fixture.main_rs_path, MAIN_RS))
        .await
        .unwrap();

    let req = LspPositionRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        position: Position { line: 0, character: 3 },
    };
    let _ = fixture.client.goto_definition(req).await.unwrap();
}

#[tokio::test]
async fn test_references_with_server() {
    let fixture = DaemonFixture::new().await;
    fixture
        .client
        .did_open(open_req(&fixture.main_rs_path, MAIN_RS))
        .await
        .unwrap();

    let req = LspPositionRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        position: Position { line: 0, character: 3 },
    };
    let _ = fixture.client.references(req).await.unwrap();
}

#[tokio::test]
async fn test_completions_with_server() {
    let fixture = DaemonFixture::new().await;
    fixture
        .client
        .did_open(open_req(&fixture.main_rs_path, MAIN_RS))
        .await
        .unwrap();

    let req = LspPositionRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        position: Position { line: 0, character: 3 },
    };
    let _ = fixture.client.completions(req).await.unwrap();
}

#[tokio::test]
async fn test_diagnostics_with_server() {
    let fixture = DaemonFixture::new().await;
    fixture
        .client
        .did_open(open_req(&fixture.main_rs_path, MAIN_RS))
        .await
        .unwrap();

    let req =
        DiagnosticsRequest { language: "rust".into(), file_path: fixture.main_rs_path.clone() };
    let _ = fixture.client.diagnostics(req).await.unwrap();
}

#[tokio::test]
async fn test_rename_with_server() {
    let fixture = DaemonFixture::new().await;
    fixture
        .client
        .did_open(open_req(
            &fixture.main_rs_path,
            "fn main() { let x = 1; }\n",
        ))
        .await
        .unwrap();

    // Give rust-analyzer a moment to process the document.
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let req = RenameRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        position: Position { line: 0, character: 17 }, // On "x"
        new_name: "renamed_var".into(),
    };
    // Rename may succeed or report an LSP error depending on readiness/position,
    // but must be a real LSP outcome — not a transport failure.
    match fixture.client.rename(req).await {
        Ok(_) | Err(ClientError::ApiError { .. }) => {}
        Err(e) => panic!("unexpected error: {e}"),
    }
}

// --- Error Handling ---

#[tokio::test]
async fn test_did_open_with_invalid_language() {
    let fixture = DaemonFixture::new().await;

    let req = DidOpenRequest {
        language: "nonexistent-language".into(),
        file_path: fixture.main_rs_path.clone(),
        language_id: "text".into(),
        version: 1,
        text: "some content".into(),
    };

    // No LSP server configured for this language → LSP_ERROR.
    let err = fixture.client.did_open(req).await.unwrap_err();
    match err {
        ClientError::ApiError { code, .. } => assert_eq!(code, "LSP_ERROR"),
        other => panic!("expected ApiError, got {other:?}"),
    }
}
