use crate::detect_workspace;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file '{path}': {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse config file '{path}': {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("Duplicate LSP config for language '{language}' in '{path}'")]
    DuplicateLanguage { path: PathBuf, language: String },

    #[error("Invalid LSP config for language '{language}' in '{path}': {reason}")]
    InvalidEntry { path: PathBuf, language: String, reason: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    #[serde(default)]
    lsp: Vec<LspServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LspServerConfig {
    pub language: String,
    #[serde(default)]
    pub file_extensions: Vec<String>,
    #[serde(default)]
    pub root_markers: Vec<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub initialization_options: Option<serde_json::Value>,
}

impl LspServerConfig {
    pub fn display_name(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.command)
    }

    fn normalize(&mut self) {
        self.language = normalize_language(&self.language);
        self.command = self.command.trim().to_string();
        self.file_extensions = dedup(
            self.file_extensions
                .iter()
                .map(|ext| normalize_extension(ext)),
        );
        self.root_markers = dedup(
            self.root_markers
                .iter()
                .map(|marker| marker.trim().to_string())
                .filter(|m| !m.is_empty()),
        );
        if let Some(display_name) = &mut self.display_name {
            let trimmed = display_name.trim();
            *display_name = trimmed.to_string();
        }
    }

    fn matches_extension(&self, extension: &str) -> bool {
        self.file_extensions.iter().any(|ext| ext == extension)
    }

    fn matches_workspace(&self, workspace: &Path) -> bool {
        self.root_markers
            .iter()
            .any(|marker| workspace.join(marker).exists())
    }
}

#[derive(Debug, Clone)]
pub struct LspRegistry {
    entries: Vec<LspServerConfig>,
}

impl LspRegistry {
    pub fn entries(&self) -> &[LspServerConfig] {
        &self.entries
    }

    pub fn find_by_language(&self, language: &str) -> Option<&LspServerConfig> {
        let language = normalize_language(language);
        self.entries.iter().find(|entry| entry.language == language)
    }

    pub fn detect_language_for_file(&self, file: &Path) -> Option<&LspServerConfig> {
        let extension = normalize_extension(file.extension()?.to_str()?);
        self.entries
            .iter()
            .find(|entry| entry.matches_extension(&extension))
    }

    pub fn detect_languages_for_workspace(&self, workspace: &Path) -> Vec<&LspServerConfig> {
        self.entries
            .iter()
            .filter(|entry| entry.matches_workspace(workspace))
            .collect()
    }
}

pub fn global_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("aifed").join("config.toml"))
}

pub fn load_lsp_registry_for_workspace(
    workspace_root: Option<&Path>,
) -> Result<LspRegistry, ConfigError> {
    let global_path = global_config_path();
    let project_path = workspace_root.map(|root| root.join("aifed.toml"));
    load_lsp_registry_from_paths(global_path.as_deref(), project_path.as_deref())
}

pub fn load_lsp_registry_for_path(path: &Path) -> Result<LspRegistry, ConfigError> {
    let workspace_root =
        detect_workspace(if path.is_dir() { path } else { path.parent().unwrap_or(path) })
            .map(|workspace| workspace.root().to_path_buf());

    load_lsp_registry_for_workspace(workspace_root.as_deref())
}

fn load_lsp_registry_from_paths(
    global_path: Option<&Path>,
    project_path: Option<&Path>,
) -> Result<LspRegistry, ConfigError> {
    let mut entries = built_in_lsp_configs();

    if let Some(path) = global_path
        && let Some(config) = load_config_file(path)?
    {
        merge_entries(&mut entries, config.lsp);
    }

    if let Some(path) = project_path
        && let Some(config) = load_config_file(path)?
    {
        merge_entries(&mut entries, config.lsp);
    }

    Ok(LspRegistry { entries })
}

fn load_config_file(path: &Path) -> Result<Option<FileConfig>, ConfigError> {
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path)
        .map_err(|source| ConfigError::Read { path: path.to_path_buf(), source })?;
    let mut config: FileConfig = toml::from_str(&content)
        .map_err(|source| ConfigError::Parse { path: path.to_path_buf(), source })?;

    validate_entries(path, &mut config.lsp)?;
    Ok(Some(config))
}

fn validate_entries(path: &Path, entries: &mut [LspServerConfig]) -> Result<(), ConfigError> {
    let mut seen = std::collections::HashSet::new();
    for entry in entries {
        entry.normalize();
        if entry.language.is_empty() {
            return Err(ConfigError::InvalidEntry {
                path: path.to_path_buf(),
                language: "<unknown>".into(),
                reason: "language must not be empty".into(),
            });
        }
        if entry.command.is_empty() {
            return Err(ConfigError::InvalidEntry {
                path: path.to_path_buf(),
                language: entry.language.clone(),
                reason: "command must not be empty".into(),
            });
        }
        if !seen.insert(entry.language.clone()) {
            return Err(ConfigError::DuplicateLanguage {
                path: path.to_path_buf(),
                language: entry.language.clone(),
            });
        }
    }

    Ok(())
}

fn merge_entries(entries: &mut Vec<LspServerConfig>, overrides: Vec<LspServerConfig>) {
    for override_entry in overrides {
        if let Some(position) = entries
            .iter()
            .position(|entry| entry.language == override_entry.language)
        {
            entries[position] = override_entry;
        } else {
            entries.push(override_entry);
        }
    }
}

fn built_in_lsp_configs() -> Vec<LspServerConfig> {
    vec![LspServerConfig {
        language: "rust".into(),
        file_extensions: vec!["rs".into()],
        root_markers: vec!["Cargo.toml".into()],
        command: "rust-analyzer".into(),
        args: vec![],
        display_name: Some("rust-analyzer".into()),
        initialization_options: Some(serde_json::json!({
            "checkOnSave": {
                "command": "clippy"
            },
            "cargo": {
                "allFeatures": true
            }
        })),
    }]
}

fn normalize_language(language: &str) -> String {
    language.trim().to_ascii_lowercase()
}

fn normalize_extension(extension: &str) -> String {
    extension
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase()
}

fn dedup<I>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();

    for value in values {
        if !value.is_empty() && seen.insert(value.clone()) {
            deduped.push(value);
        }
    }

    deduped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn built_in_registry_detects_rust() {
        let dir = tempfile::tempdir().unwrap();
        write_file(
            &dir.path().join("Cargo.toml"),
            "[package]\nname='demo'\nversion='0.1.0'\n",
        );
        write_file(&dir.path().join("src/main.rs"), "fn main() {}\n");

        let registry = load_lsp_registry_from_paths(None, None).unwrap();
        let file_language = registry
            .detect_language_for_file(&dir.path().join("src/main.rs"))
            .unwrap();
        let workspace_languages = registry.detect_languages_for_workspace(dir.path());

        assert_eq!(file_language.language, "rust");
        assert_eq!(workspace_languages.len(), 1);
        assert_eq!(workspace_languages[0].language, "rust");
    }

    #[test]
    fn project_config_replaces_builtin_entry() {
        let dir = tempfile::tempdir().unwrap();
        let project_config = dir.path().join("aifed.toml");
        write_file(
            &project_config,
            r#"
[[lsp]]
language = "rust"
file_extensions = ["rs", "rust"]
root_markers = ["Cargo.toml", "rust-project.json"]
command = "custom-rust-analyzer"
args = ["--stdio"]
"#,
        );

        let registry = load_lsp_registry_from_paths(None, Some(&project_config)).unwrap();
        let rust = registry.find_by_language("rust").unwrap();

        assert_eq!(rust.command, "custom-rust-analyzer");
        assert_eq!(rust.args, vec!["--stdio"]);
        assert_eq!(rust.file_extensions, vec!["rs", "rust"]);
        assert_eq!(rust.root_markers, vec!["Cargo.toml", "rust-project.json"]);
    }

    #[test]
    fn project_config_adds_custom_language() {
        let dir = tempfile::tempdir().unwrap();
        let project_config = dir.path().join("aifed.toml");
        let custom_file = dir.path().join("main.foo");
        write_file(&custom_file, "hello\n");
        write_file(
            &project_config,
            r#"
[[lsp]]
language = "foo"
file_extensions = ["foo"]
root_markers = ["foo.mod"]
command = "foo-lsp"
"#,
        );

        let registry = load_lsp_registry_from_paths(None, Some(&project_config)).unwrap();
        let foo = registry.detect_language_for_file(&custom_file).unwrap();

        assert_eq!(foo.language, "foo");
        assert_eq!(foo.command, "foo-lsp");
    }

    #[test]
    fn project_config_overrides_global_config() {
        let dir = tempfile::tempdir().unwrap();
        let global_config = dir.path().join("global.toml");
        let project_config = dir.path().join("project.toml");
        write_file(
            &global_config,
            r#"
[[lsp]]
language = "rust"
file_extensions = ["rs"]
root_markers = ["Cargo.toml"]
command = "global-rust-analyzer"
"#,
        );
        write_file(
            &project_config,
            r#"
[[lsp]]
language = "rust"
file_extensions = ["rs"]
root_markers = ["Cargo.toml"]
command = "project-rust-analyzer"
"#,
        );

        let registry =
            load_lsp_registry_from_paths(Some(&global_config), Some(&project_config)).unwrap();
        assert_eq!(
            registry.find_by_language("rust").unwrap().command,
            "project-rust-analyzer"
        );
    }

    #[test]
    fn duplicate_language_in_single_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let project_config = dir.path().join("aifed.toml");
        write_file(
            &project_config,
            r#"
[[lsp]]
language = "rust"
file_extensions = ["rs"]
root_markers = ["Cargo.toml"]
command = "rust-analyzer"

[[lsp]]
language = "rust"
file_extensions = ["rs"]
root_markers = ["Cargo.toml"]
command = "custom-rust-analyzer"
"#,
        );

        let error = load_lsp_registry_from_paths(None, Some(&project_config)).unwrap_err();
        assert!(matches!(error, ConfigError::DuplicateLanguage { .. }));
    }
}
