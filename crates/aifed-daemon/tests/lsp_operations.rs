//! LSP operations integration tests
//!
//! Tests for LSP HTTP API endpoints:
//! - Document sync: didOpen, didChange, didClose
//! - LSP queries: hover, definition, references, completions, diagnostics, rename
//!
//! Note: The daemon auto-starts LSP servers for detected languages in the workspace.
//! For Rust workspaces, rust-analyzer is started automatically.

mod common;

use aifed_common::{
    ApiResponse, ContentChange, DiagnosticsRequest, DidChangeRequest, DidCloseRequest,
    DidOpenRequest, HoverRequest, LspPositionRequest, Position, RenameRequest,
};
use common::DaemonFixture;
use hyper::StatusCode;

// --- Document Synchronization Tests (with server) ---
// The daemon auto-starts rust-analyzer for Rust workspaces

#[tokio::test]
async fn test_did_open_with_server() {
    let fixture = DaemonFixture::new().await;

    let req = DidOpenRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        language_id: "rust".into(),
        version: 1,
        text: "fn main() {}".into(),
    };

    let resp = fixture
        .client
        .post("/api/v1/lsp/didOpen", &req)
        .await
        .unwrap();
    // Should succeed since rust-analyzer is auto-started
    assert!(resp.is_success());

    let json: ApiResponse<serde_json::Value> = resp.json();
    assert!(json.success);
}

#[tokio::test]
async fn test_did_change_with_server() {
    let fixture = DaemonFixture::new().await;

    // First open the document
    let open_req = DidOpenRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        language_id: "rust".into(),
        version: 1,
        text: "fn main() {}".into(),
    };
    fixture
        .client
        .post("/api/v1/lsp/didOpen", &open_req)
        .await
        .unwrap();

    // Then send a change
    let change_req = DidChangeRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        version: 2,
        content_changes: vec![ContentChange {
            range: None, // Full document sync
            text: "fn main() { println!(\"updated\"); }".into(),
        }],
    };
    let resp = fixture
        .client
        .post("/api/v1/lsp/didChange", &change_req)
        .await
        .unwrap();
    // Should succeed since rust-analyzer is auto-started
    assert!(resp.is_success());
}

#[tokio::test]
async fn test_did_close_with_server() {
    let fixture = DaemonFixture::new().await;

    // Open first
    let open_req = DidOpenRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        language_id: "rust".into(),
        version: 1,
        text: "fn main() {}".into(),
    };
    fixture
        .client
        .post("/api/v1/lsp/didOpen", &open_req)
        .await
        .unwrap();

    // Then close
    let close_req =
        DidCloseRequest { language: "rust".into(), file_path: fixture.main_rs_path.clone() };
    let resp = fixture
        .client
        .post("/api/v1/lsp/didClose", &close_req)
        .await
        .unwrap();
    // Should succeed since rust-analyzer is auto-started
    assert!(resp.is_success());

    let json: ApiResponse<serde_json::Value> = resp.json();
    assert!(json.success);
}

#[tokio::test]
async fn test_document_sync_full_workflow() {
    let fixture = DaemonFixture::new().await;

    // 1. Open document
    let open_req = DidOpenRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        language_id: "rust".into(),
        version: 1,
        text: "fn main() {}".into(),
    };
    let resp = fixture
        .client
        .post("/api/v1/lsp/didOpen", &open_req)
        .await
        .unwrap();
    assert!(resp.is_success());

    // 2. Multiple changes
    for version in 2..=3 {
        let change_req = DidChangeRequest {
            language: "rust".into(),
            file_path: fixture.main_rs_path.clone(),
            version,
            content_changes: vec![ContentChange {
                range: None,
                text: format!("// v{}\nfn main() {{}}", version),
            }],
        };
        let resp = fixture
            .client
            .post("/api/v1/lsp/didChange", &change_req)
            .await
            .unwrap();
        assert!(resp.is_success());
    }

    // 3. Close
    let close_req =
        DidCloseRequest { language: "rust".into(), file_path: fixture.main_rs_path.clone() };
    let resp = fixture
        .client
        .post("/api/v1/lsp/didClose", &close_req)
        .await
        .unwrap();
    assert!(resp.is_success());
}

// --- LSP Query Tests (with server) ---
// The daemon auto-starts rust-analyzer for Rust workspaces

#[tokio::test]
async fn test_hover_with_server() {
    let fixture = DaemonFixture::new().await;

    // First open the document
    let open_req = DidOpenRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        language_id: "rust".into(),
        version: 1,
        text: "fn main() {}".into(),
    };
    fixture
        .client
        .post("/api/v1/lsp/didOpen", &open_req)
        .await
        .unwrap();

    let req = HoverRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        position: Position { line: 0, character: 3 },
    };

    let resp = fixture
        .client
        .post("/api/v1/lsp/hover", &req)
        .await
        .unwrap();
    // Should succeed (may return empty hover if no info available)
    assert!(resp.is_success());

    let json: ApiResponse<serde_json::Value> = resp.json();
    assert!(json.success);
}

#[tokio::test]
async fn test_definition_with_server() {
    let fixture = DaemonFixture::new().await;

    // First open the document
    let open_req = DidOpenRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        language_id: "rust".into(),
        version: 1,
        text: "fn main() {}".into(),
    };
    fixture
        .client
        .post("/api/v1/lsp/didOpen", &open_req)
        .await
        .unwrap();

    let req = LspPositionRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        position: Position { line: 0, character: 3 }, // On "fn"
    };

    let resp = fixture
        .client
        .post("/api/v1/lsp/definition", &req)
        .await
        .unwrap();
    // Should succeed (may return empty locations)
    assert!(resp.is_success());

    let json: ApiResponse<serde_json::Value> = resp.json();
    assert!(json.success);
}

#[tokio::test]
async fn test_references_with_server() {
    let fixture = DaemonFixture::new().await;

    // First open the document
    let open_req = DidOpenRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        language_id: "rust".into(),
        version: 1,
        text: "fn main() {}".into(),
    };
    fixture
        .client
        .post("/api/v1/lsp/didOpen", &open_req)
        .await
        .unwrap();

    let req = LspPositionRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        position: Position { line: 0, character: 3 },
    };

    let resp = fixture
        .client
        .post("/api/v1/lsp/references", &req)
        .await
        .unwrap();
    // Should succeed (may return empty references)
    assert!(resp.is_success());

    let json: ApiResponse<serde_json::Value> = resp.json();
    assert!(json.success);
}

#[tokio::test]
async fn test_completions_with_server() {
    let fixture = DaemonFixture::new().await;

    // First open the document
    let open_req = DidOpenRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        language_id: "rust".into(),
        version: 1,
        text: "fn main() {}".into(),
    };
    fixture
        .client
        .post("/api/v1/lsp/didOpen", &open_req)
        .await
        .unwrap();

    let req = LspPositionRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        position: Position { line: 0, character: 3 },
    };

    let resp = fixture
        .client
        .post("/api/v1/lsp/completions", &req)
        .await
        .unwrap();
    // Should succeed (may return empty completions)
    assert!(resp.is_success());

    let json: ApiResponse<serde_json::Value> = resp.json();
    assert!(json.success);
}

#[tokio::test]
async fn test_diagnostics_with_server() {
    let fixture = DaemonFixture::new().await;

    // First open the document
    let open_req = DidOpenRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        language_id: "rust".into(),
        version: 1,
        text: "fn main() {}".into(),
    };
    fixture
        .client
        .post("/api/v1/lsp/didOpen", &open_req)
        .await
        .unwrap();

    let req =
        DiagnosticsRequest { language: "rust".into(), file_path: fixture.main_rs_path.clone() };

    let resp = fixture
        .client
        .post("/api/v1/lsp/diagnostics", &req)
        .await
        .unwrap();
    // Should succeed (may return empty diagnostics)
    assert!(resp.is_success());

    let json: ApiResponse<serde_json::Value> = resp.json();
    assert!(json.success);
}

#[tokio::test]
async fn test_rename_with_server() {
    let fixture = DaemonFixture::new().await;

    // First open the document with a variable that can be renamed
    let open_req = DidOpenRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        language_id: "rust".into(),
        version: 1,
        text: "fn main() { let x = 1; }".into(),
    };
    fixture
        .client
        .post("/api/v1/lsp/didOpen", &open_req)
        .await
        .unwrap();

    // Give rust-analyzer a moment to process the document
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let req = RenameRequest {
        language: "rust".into(),
        file_path: fixture.main_rs_path.clone(),
        position: Position { line: 0, character: 17 }, // On "x" variable
        new_name: "renamed_var".into(),
    };

    let resp = fixture
        .client
        .post("/api/v1/lsp/rename", &req)
        .await
        .unwrap();
    // Rename may succeed or fail depending on LSP server readiness and position
    // The important thing is that the handler is properly implemented and returns a response
    // (not NOT_IMPLEMENTED)
    if resp.is_success() {
        let json: ApiResponse<serde_json::Value> = resp.json();
        assert!(json.success);
    } else {
        // If it fails, it should be an LSP error, not NOT_IMPLEMENTED
        assert_ne!(resp.status, StatusCode::NOT_IMPLEMENTED);
    }
}

// --- Error Handling Tests ---

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

    // Returns LSP_ERROR because no LSP server is configured for this language
    let resp = fixture
        .client
        .post("/api/v1/lsp/didOpen", &req)
        .await
        .unwrap();
    assert_eq!(resp.status, StatusCode::INTERNAL_SERVER_ERROR);

    let json: ApiResponse<serde_json::Value> = resp.json();
    assert!(!json.success);
    assert_eq!(json.error.unwrap().code, "LSP_ERROR");
}
