//! Unit tests for LSP client using mock

use super::*;
use crate::lsp::mock::MockLspClient;
use lsp_types::*;

#[tokio::test]
async fn test_initialize_returns_server_info() {
    let mut client = MockLspClient::new();

    let result = client
        .initialize(InitializeParams {
            capabilities: ClientCapabilities::default(),
            ..InitializeParams::default()
        })
        .await
        .unwrap();

    assert!(result.server_info.is_some());
    assert_eq!(result.server_info.unwrap().name, "mock-server");
}

#[tokio::test]
async fn test_hover_returns_configured_response() {
    let mut client = MockLspClient::new();

    client.set_response(
        "hover",
        serde_json::json!({
            "contents": {
                "kind": "markdown",
                "value": "fn add(a: i32, b: i32) -> i32"
            },
            "range": null
        }),
    );

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: Url::parse("file:///test.rs").unwrap() },
            position: Position { line: 0, character: 5 },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };

    let result = client.hover(params).await.unwrap();
    assert!(result.is_some());

    let hover = result.unwrap();
    match hover.contents {
        HoverContents::Markup(markup) => {
            assert!(markup.value.contains("fn add"));
        }
        _ => panic!("Expected markup content"),
    }
}

#[tokio::test]
async fn test_hover_returns_none_when_not_configured() {
    let mut client = MockLspClient::new();

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: Url::parse("file:///test.rs").unwrap() },
            position: Position { line: 0, character: 5 },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };

    let result = client.hover(params).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_goto_definition_returns_locations() {
    let mut client = MockLspClient::new();

    client.set_response(
        "definition",
        serde_json::json!([
            {
                "uri": "file:///src/lib.rs",
                "range": {
                    "start": { "line": 10, "character": 0 },
                    "end": { "line": 10, "character": 20 }
                }
            }
        ]),
    );

    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: Url::parse("file:///test.rs").unwrap() },
            position: Position { line: 5, character: 10 },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = client.goto_definition(params).await.unwrap();
    assert!(result.is_some());

    match result.unwrap() {
        GotoDefinitionResponse::Array(locations) => {
            assert_eq!(locations.len(), 1);
            assert_eq!(locations[0].uri.as_str(), "file:///src/lib.rs");
        }
        _ => panic!("Expected array response"),
    }
}

#[tokio::test]
async fn test_references_returns_locations() {
    let mut client = MockLspClient::new();

    client.set_response(
        "references",
        serde_json::json!([
            {
                "uri": "file:///src/lib.rs",
                "range": {
                    "start": { "line": 5, "character": 0 },
                    "end": { "line": 5, "character": 10 }
                }
            },
            {
                "uri": "file:///src/main.rs",
                "range": {
                    "start": { "line": 20, "character": 5 },
                    "end": { "line": 20, "character": 15 }
                }
            }
        ]),
    );

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: Url::parse("file:///test.rs").unwrap() },
            position: Position { line: 0, character: 0 },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: ReferenceContext { include_declaration: true },
    };

    let result = client.references(params).await.unwrap();
    assert!(result.is_some());

    let locations = result.unwrap();
    assert_eq!(locations.len(), 2);
}

#[tokio::test]
async fn test_completion_returns_items() {
    let mut client = MockLspClient::new();

    client.set_response(
        "completion",
        serde_json::json!([
            {
                "label": "add",
                "kind": 3, // Function
                "detail": "fn add(a: i32, b: i32) -> i32"
            },
            {
                "label": "subtract",
                "kind": 3,
                "detail": "fn subtract(a: i32, b: i32) -> i32"
            }
        ]),
    );

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: Url::parse("file:///test.rs").unwrap() },
            position: Position { line: 0, character: 5 },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = client.completion(params).await.unwrap();
    assert!(result.is_some());

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].label, "add");
        }
        CompletionResponse::List(_) => panic!("Expected array response"),
    }
}

#[tokio::test]
async fn test_rename_returns_workspace_edit() {
    let mut client = MockLspClient::new();

    client.set_response(
        "rename",
        serde_json::json!({
            "changes": {
                "file:///src/lib.rs": [
                    {
                        "range": {
                            "start": { "line": 0, "character": 3 },
                            "end": { "line": 0, "character": 6 }
                        },
                        "newText": "new_name"
                    }
                ]
            }
        }),
    );

    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: Url::parse("file:///test.rs").unwrap() },
            position: Position { line: 0, character: 3 },
        },
        new_name: "new_name".to_string(),
        work_done_progress_params: WorkDoneProgressParams::default(),
    };

    let result = client.rename(params).await.unwrap();
    assert!(result.is_some());

    let edit = result.unwrap();
    assert!(edit.changes.is_some());
    assert_eq!(edit.changes.unwrap().len(), 1);
}

#[tokio::test]
async fn test_did_open_succeeds() {
    let mut client = MockLspClient::new();

    let params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: Url::parse("file:///test.rs").unwrap(),
            language_id: "rust".to_string(),
            version: 1,
            text: "fn main() {}".to_string(),
        },
    };

    let result = client.did_open(params).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_did_change_succeeds() {
    let mut client = MockLspClient::new();

    let params = DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier {
            uri: Url::parse("file:///test.rs").unwrap(),
            version: 2,
        },
        content_changes: vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "fn main() { println!() }".to_string(),
        }],
    };

    let result = client.did_change(params).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_did_close_succeeds() {
    let mut client = MockLspClient::new();

    let params = DidCloseTextDocumentParams {
        text_document: TextDocumentIdentifier { uri: Url::parse("file:///test.rs").unwrap() },
    };

    let result = client.did_close(params).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_shutdown_succeeds() {
    let mut client = MockLspClient::new();

    // Initialize first
    client
        .initialize(InitializeParams {
            capabilities: ClientCapabilities::default(),
            ..InitializeParams::default()
        })
        .await
        .unwrap();

    let result = client.shutdown(true).await;
    assert!(result.is_ok());
}
