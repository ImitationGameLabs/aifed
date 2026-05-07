//! Config-backed LSP server configurations.

use crate::lsp::LanguageServerConfig;
use aifed_common::LspServerConfig;
use std::path::Path;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct ConfiguredLanguageServerConfig {
    config: LspServerConfig,
}

impl ConfiguredLanguageServerConfig {
    pub fn new(config: LspServerConfig) -> Self {
        Self { config }
    }
}

impl LanguageServerConfig for ConfiguredLanguageServerConfig {
    fn language_id(&self) -> &str {
        &self.config.language
    }

    fn spawn_command(&self, _workspace_root: &Path) -> Command {
        let mut command = Command::new(&self.config.command);
        command.args(&self.config.args);
        command
    }

    fn initialization_options(&self) -> Option<serde_json::Value> {
        self.config.initialization_options.clone()
    }

    fn display_name(&self) -> &str {
        self.config.display_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_backed_server_preserves_language_and_display_name() {
        let config = LspServerConfig {
            language: "rust".into(),
            file_extensions: vec!["rs".into()],
            root_markers: vec!["Cargo.toml".into()],
            command: "rust-analyzer".into(),
            args: vec!["--stdio".into()],
            display_name: Some("rust-analyzer".into()),
            initialization_options: None,
        };

        let server = ConfiguredLanguageServerConfig::new(config);
        assert_eq!(server.language_id(), "rust");
        assert_eq!(server.display_name(), "rust-analyzer");
    }
}
