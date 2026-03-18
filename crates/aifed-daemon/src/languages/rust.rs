//! Rust language server configuration (rust-analyzer)

use crate::lsp::LanguageServerConfig;
use std::path::Path;
use tokio::process::Command;

/// rust-analyzer language server configuration
pub struct RustAnalyzerConfig;

impl LanguageServerConfig for RustAnalyzerConfig {
    fn language_id(&self) -> &str {
        "rust"
    }

    fn spawn_command(&self, _workspace_root: &Path) -> Command {
        Command::new("rust-analyzer")
    }

    fn initialization_options(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "checkOnSave": {
                "command": "clippy"
            },
            "cargo": {
                "allFeatures": true
            }
        }))
    }

    fn display_name(&self) -> &str {
        "rust-analyzer"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_id() {
        let config = RustAnalyzerConfig;
        assert_eq!(config.language_id(), "rust");
    }

    #[test]
    fn test_display_name() {
        let config = RustAnalyzerConfig;
        assert_eq!(config.display_name(), "rust-analyzer");
    }

    #[test]
    fn test_initialization_options() {
        let config = RustAnalyzerConfig;
        let options = config.initialization_options().unwrap();
        assert!(options.is_object());
    }
}
