//! LSP (Language Server Protocol) client implementation
//!
//! This module provides a generic LSP client implementation with:
//! - Transport abstraction for JSON-RPC communication
//! - Generic LSP client trait for language server operations
//! - LanguageServerManager for managing multiple language server instances
//! - Progress tracking for work done notifications

pub mod client;
pub mod manager;
pub mod progress;
pub mod protocol;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use aifed_common::ServerState;
pub use client::{LanguageServerConfig, LspClient};
pub use manager::{LanguageServerManager, ServerStatus};
