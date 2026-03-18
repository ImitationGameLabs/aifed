//! Mock LSP client for testing

use async_trait::async_trait;
use lsp_types::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::LspClient;
use super::progress::ProgressInfo;
use crate::error::{Error, Result};

/// Mock LSP client that returns pre-configured responses
pub struct MockLspClient {
    responses: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    initialized: bool,
}

impl MockLspClient {
    pub fn new() -> Self {
        Self { responses: Arc::new(Mutex::new(HashMap::new())), initialized: false }
    }

    /// Configure a response for a specific method
    pub fn set_response(&mut self, method: &str, result: serde_json::Value) {
        self.responses.lock().unwrap().insert(method.to_string(), result);
    }

    /// Get response for a method, returns None if not configured
    fn get_response(&self, method: &str) -> Option<serde_json::Value> {
        self.responses.lock().unwrap().get(method).cloned()
    }

    /// Check if error is configured for a method
    fn has_error(&self, method: &str) -> bool {
        self.responses.lock().unwrap().contains_key(&format!("{}_error", method))
    }

    /// Get error message if configured
    fn get_error_message(&self, method: &str) -> Option<String> {
        self.responses
            .lock()
            .unwrap()
            .get(&format!("{}_error", method))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
}

impl Default for MockLspClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LspClient for MockLspClient {
    async fn initialize(&mut self, _params: InitializeParams) -> Result<InitializeResult> {
        if self.has_error("initialize") {
            return Err(Error::Transport(self.get_error_message("initialize").unwrap_or_default()));
        }
        self.initialized = true;

        // Return a default server info
        Ok(InitializeResult {
            capabilities: ServerCapabilities::default(),
            server_info: Some(ServerInfo { name: "mock-server".to_string(), version: None }),
        })
    }

    async fn initialized(&mut self) -> Result<()> {
        Ok(())
    }

    async fn shutdown(&mut self, _force: bool) -> Result<()> {
        self.initialized = false;
        Ok(())
    }

    async fn goto_definition(
        &mut self,
        _params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        if self.has_error("definition") {
            return Err(Error::Transport(self.get_error_message("definition").unwrap_or_default()));
        }

        let response = self.get_response("definition");
        match response {
            Some(v) => {
                let locations: Vec<Location> =
                    serde_json::from_value(v).map_err(Error::JsonDeserialize)?;
                Ok(Some(GotoDefinitionResponse::Array(locations)))
            }
            None => Ok(None),
        }
    }

    async fn references(&mut self, _params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        if self.has_error("references") {
            return Err(Error::Transport(self.get_error_message("references").unwrap_or_default()));
        }

        let response = self.get_response("references");
        match response {
            Some(v) => {
                let locations: Vec<Location> =
                    serde_json::from_value(v).map_err(Error::JsonDeserialize)?;
                Ok(Some(locations))
            }
            None => Ok(None),
        }
    }

    async fn hover(&mut self, _params: HoverParams) -> Result<Option<Hover>> {
        if self.has_error("hover") {
            return Err(Error::Transport(self.get_error_message("hover").unwrap_or_default()));
        }

        let response = self.get_response("hover");
        match response {
            Some(v) => {
                let hover: Hover = serde_json::from_value(v).map_err(Error::JsonDeserialize)?;
                Ok(Some(hover))
            }
            None => Ok(None),
        }
    }

    async fn completion(
        &mut self,
        _params: CompletionParams,
    ) -> Result<Option<CompletionResponse>> {
        if self.has_error("completion") {
            return Err(Error::Transport(self.get_error_message("completion").unwrap_or_default()));
        }

        let response = self.get_response("completion");
        match response {
            Some(v) => {
                let items: Vec<CompletionItem> =
                    serde_json::from_value(v).map_err(Error::JsonDeserialize)?;
                Ok(Some(CompletionResponse::Array(items)))
            }
            None => Ok(None),
        }
    }

    async fn rename(&mut self, _params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        if self.has_error("rename") {
            return Err(Error::Transport(self.get_error_message("rename").unwrap_or_default()));
        }

        let response = self.get_response("rename");
        match response {
            Some(v) => {
                let edit: WorkspaceEdit =
                    serde_json::from_value(v).map_err(Error::JsonDeserialize)?;
                Ok(Some(edit))
            }
            None => Ok(None),
        }
    }

    async fn diagnostic(
        &mut self,
        _params: DocumentDiagnosticParams,
    ) -> Result<DocumentDiagnosticReportResult> {
        if self.has_error("diagnostic") {
            return Err(Error::Transport(self.get_error_message("diagnostic").unwrap_or_default()));
        }

        // Return empty report by default
        Ok(DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(
            RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: None,
                    items: vec![],
                },
            },
        )))
    }

    async fn did_open(&mut self, _params: DidOpenTextDocumentParams) -> Result<()> {
        Ok(())
    }

    async fn did_change(&mut self, _params: DidChangeTextDocumentParams) -> Result<()> {
        Ok(())
    }

    async fn did_close(&mut self, _params: DidCloseTextDocumentParams) -> Result<()> {
        Ok(())
    }

    async fn get_progress(&self) -> Vec<ProgressInfo> {
        // Mock client has no progress tracking
        Vec::new()
    }
}
