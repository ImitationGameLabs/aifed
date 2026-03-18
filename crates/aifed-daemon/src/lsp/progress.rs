//! Progress tracking for LSP work done notifications
//!
//! This module handles the LSP `$/progress` notification flow:
//! 1. Server sends `window/workDoneProgress/create` request with a token
//! 2. Server sends `$/progress` notifications with Begin/Report/End states
//! 3. Tracker maintains state for progress tokens

use lsp_types::{ProgressParams, ProgressToken, WorkDoneProgress};
use serde::Serialize;
use std::collections::HashMap;
use tokio::sync::Mutex;

/// State of a progress operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressState {
    /// Progress has started (Begin received)
    Active,
    /// Progress has completed (End received)
    Ended,
}

/// Progress information exposed to API consumers
#[derive(Debug, Clone, Serialize)]
pub struct ProgressInfo {
    pub title: Option<String>,
    pub message: Option<String>,
    pub percentage: Option<u32>,
}

/// Stored information about a progress token
#[derive(Debug, Clone)]
struct TokenInfo {
    state: ProgressState,
    title: Option<String>,
    message: Option<String>,
    percentage: Option<u32>,
}

/// Internal state for progress tracking
struct Inner {
    /// Known progress tokens and their info
    tokens: HashMap<ProgressToken, TokenInfo>,
}

/// Tracker for LSP progress notifications
pub struct ProgressTracker {
    inner: Mutex<Inner>,
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressTracker {
    /// Create a new progress tracker
    pub fn new() -> Self {
        Self { inner: Mutex::new(Inner { tokens: HashMap::new() }) }
    }

    /// Register a new progress token (from window/workDoneProgress/create)
    pub async fn register_token(&self, token: ProgressToken) {
        let mut inner = self.inner.lock().await;
        inner.tokens.entry(token).or_insert_with(|| TokenInfo {
            state: ProgressState::Active,
            title: None,
            message: None,
            percentage: None,
        });
    }

    /// Handle a `$/progress` notification
    pub async fn handle_progress(&self, params: ProgressParams) {
        let token = params.token.clone();

        let mut inner = self.inner.lock().await;

        match &params.value {
            lsp_types::ProgressParamsValue::WorkDone(wdp) => match wdp {
                WorkDoneProgress::Begin(begin) => {
                    inner.tokens.insert(
                        token,
                        TokenInfo {
                            state: ProgressState::Active,
                            title: Some(begin.title.clone()),
                            message: begin.message.clone(),
                            percentage: begin.percentage,
                        },
                    );
                }
                WorkDoneProgress::Report(report) => {
                    if let Some(info) = inner.tokens.get_mut(&token) {
                        info.message = report.message.clone().or(info.message.clone());
                        info.percentage = report.percentage.or(info.percentage);
                    }
                }
                WorkDoneProgress::End(_end) => {
                    if let Some(info) = inner.tokens.get_mut(&token) {
                        info.state = ProgressState::Ended;
                    }
                }
            },
        }
    }

    /// Get all active progress information
    pub async fn get_active_progress(&self) -> Vec<ProgressInfo> {
        let inner = self.inner.lock().await;
        inner
            .tokens
            .values()
            .filter(|info| matches!(info.state, ProgressState::Active))
            .map(|info| ProgressInfo {
                title: info.title.clone(),
                message: info.message.clone(),
                percentage: info.percentage,
            })
            .collect()
    }
}
