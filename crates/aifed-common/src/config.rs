use crate::detect_workspace;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

const DEFAULT_CONFIG_TOML: &str = include_str!("default-config.toml");

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

    // Wording is language-neutral: the same `[[language]]` name may appear in
    // either an `[[lsp]]` entry or a `[[language]]` overlay.
    #[error("Duplicate entry for language '{language}' in '{path}'")]
    DuplicateLanguage { path: PathBuf, language: String },

    #[error("Invalid LSP config for language '{language}' in '{path}': {reason}")]
    InvalidEntry { path: PathBuf, language: String, reason: String },

    #[error("Failed to write default config to '{path}': {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Declared indentation style for a language. When set on a [[language]]
/// overlay it skips detection and asserts the file matches the convention
/// (a mismatch is a hard error, not a silent rewrite). Per-variant rename
/// keeps the lowercase TOML form without a rename_all, which the codebase
/// does not use elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndentStyleConfig {
    #[serde(rename = "tab")]
    Tab,
    #[serde(rename = "space")]
    Space,
}

/// Global configuration for the @N indent directive (see aifed::indent).
/// assist gates the whole feature; it defaults to true so the directive is
/// honored unless a project explicitly opts out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndentConfig {
    #[serde(default = "default_assist_true")]
    pub assist: bool,
}

impl Default for IndentConfig {
    fn default() -> Self {
        Self { assist: true }
    }
}

fn default_assist_true() -> bool {
    true
}

/// Top-level config file: a list of LSP server entries plus a list of language
/// extension overlays. Both are `[[array]]` tables in TOML.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    #[serde(default)]
    lsp: Vec<LspServerConfig>,
    #[serde(default)]
    language: Vec<LanguageConfig>,
    #[serde(default)]
    indent: IndentConfig,
}

/// An LSP server entry. References a language by name only — extension matching
/// is owned by the language registry (`aifed::language`), not by this entry, so
/// a language can have an outline grammar without an LSP (e.g. Markdown).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LspServerConfig {
    pub language: String,
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

    fn matches_workspace(&self, workspace: &Path) -> bool {
        self.root_markers
            .iter()
            .any(|marker| workspace.join(marker).exists())
    }
}

/// A language extension overlay. Layers on top of a grammar's built-in default
/// extensions (see `aifed::language::GRAMMAR_DEFAULTS`):
///
/// `effective = (grammar_defaults ∪ additional_extensions) − exclude_extensions`
///
/// For a language with no shipped grammar, `additional_extensions` is its full
/// extension set — outline resolves it but reports "no outline grammar".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageConfig {
    pub language: String,
    #[serde(default)]
    pub additional_extensions: Vec<String>,
    #[serde(default)]
    pub exclude_extensions: Vec<String>,
    /// Override indent assist for this language. None inherits the global
    /// [indent] assist; Some(false) disables the directive for the language.
    #[serde(default)]
    pub indent_assist: Option<bool>,
    /// Declared indent style; skips detection and asserts file consistency.
    #[serde(default)]
    pub indent_style: Option<IndentStyleConfig>,
    /// Declared spaces per level; pairs with indent_style = "space".
    #[serde(default)]
    pub indent_width: Option<u32>,
}

impl LanguageConfig {
    fn normalize(&mut self) {
        self.language = normalize_language(&self.language);
        self.additional_extensions = dedup(
            self.additional_extensions
                .iter()
                .map(|e| normalize_extension(e)),
        );
        self.exclude_extensions = dedup(
            self.exclude_extensions
                .iter()
                .map(|e| normalize_extension(e)),
        );
    }
}

/// The loaded, merged configuration: LSP server entries plus language extension
/// overlays. Built fresh per command by merging the global config
/// (`$AIFED_CONFIG_DIR/config.toml` or `~/.config/aifed/config.toml`) with the
/// project config (`<workspace>/aifed.toml`); project entries wholesale-replace
/// same-name global entries.
#[derive(Debug, Clone)]
pub struct Registry {
    entries: Vec<LspServerConfig>,
    language_overlays: Vec<LanguageConfig>,
    indent: IndentConfig,
}

impl Registry {
    /// Construct directly from parts, bypassing file loading. For tests and
    /// embedding only — inputs are NOT normalized or validated; callers must
    /// pre-normalize language names and extensions.
    pub fn from_parts(
        entries: Vec<LspServerConfig>,
        language_overlays: Vec<LanguageConfig>,
        indent: IndentConfig,
    ) -> Self {
        Self { entries, language_overlays, indent }
    }

    pub fn entries(&self) -> &[LspServerConfig] {
        &self.entries
    }

    /// `[[language]]` overlays, global-then-project merged (project replaces
    /// same-name global). Consumed by `aifed::language::LanguageResolver`.
    pub fn language_overlays(&self) -> &[LanguageConfig] {
        &self.language_overlays
    }

    /// Resolved global [indent] settings (project overrides global).
    pub fn indent(&self) -> &IndentConfig {
        &self.indent
    }

    pub fn find_by_language(&self, language: &str) -> Option<&LspServerConfig> {
        let language = normalize_language(language);
        self.entries.iter().find(|entry| entry.language == language)
    }

    pub fn detect_languages_for_workspace(&self, workspace: &Path) -> Vec<&LspServerConfig> {
        self.entries
            .iter()
            .filter(|entry| entry.matches_workspace(workspace))
            .collect()
    }
}

pub fn global_config_path() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("AIFED_CONFIG_DIR")
        && !dir.is_empty()
    {
        return Some(PathBuf::from(dir).join("config.toml"));
    }
    dirs::config_dir().map(|dir| dir.join("aifed").join("config.toml"))
}

/// Write the default config to the global path if it does not exist yet.
///
/// NixOS users with home-manager will already have the file via xdg.configFile,
/// so this is a no-op. Non-Nix users get the default config on first run.
pub fn ensure_default_config() -> Result<(), ConfigError> {
    if let Some(path) = global_config_path()
        && !path.exists()
    {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|source| ConfigError::Write { path: parent.to_path_buf(), source })?;
        }
        fs::write(&path, DEFAULT_CONFIG_TOML)
            .map_err(|source| ConfigError::Write { path: path.clone(), source })?;
    }
    Ok(())
}

pub fn load_registry_for_workspace(workspace_root: Option<&Path>) -> Result<Registry, ConfigError> {
    let global_path = global_config_path();
    let project_path = workspace_root.map(|root| root.join("aifed.toml"));
    load_registry_from_paths(global_path.as_deref(), project_path.as_deref())
}

pub fn load_registry_for_path(path: &Path) -> Result<Registry, ConfigError> {
    let workspace_root =
        detect_workspace(if path.is_dir() { path } else { path.parent().unwrap_or(path) })
            .map(|workspace| workspace.root().to_path_buf());

    load_registry_for_workspace(workspace_root.as_deref())
}

fn load_registry_from_paths(
    global_path: Option<&Path>,
    project_path: Option<&Path>,
) -> Result<Registry, ConfigError> {
    let mut entries = Vec::new();
    let mut language_overlays = Vec::new();
    let mut indent = IndentConfig::default();

    if let Some(path) = global_path
        && let Some(config) = load_config_file(path)?
    {
        merge_entries(&mut entries, config.lsp);
        merge_language_entries(&mut language_overlays, config.language);
        indent = config.indent;
    }

    if let Some(path) = project_path
        && let Some(config) = load_config_file(path)?
    {
        merge_entries(&mut entries, config.lsp);
        merge_language_entries(&mut language_overlays, config.language);
        indent = config.indent;
    }

    Ok(Registry { entries, language_overlays, indent })
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
    validate_language_entries(path, &mut config.language)?;
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

/// Reject duplicate `language` names within a single file's `[[language]]` list.
fn validate_language_entries(
    path: &Path,
    entries: &mut [LanguageConfig],
) -> Result<(), ConfigError> {
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
        if !seen.insert(entry.language.clone()) {
            return Err(ConfigError::DuplicateLanguage {
                path: path.to_path_buf(),
                language: entry.language.clone(),
            });
        }
        if entry.indent_style == Some(IndentStyleConfig::Space) && entry.indent_width == Some(0) {
            return Err(ConfigError::InvalidEntry {
                path: path.to_path_buf(),
                language: entry.language.clone(),
                reason: "indent_width must be at least 1 for indent_style space".into(),
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

/// Project entries wholesale-replace same-name global entries (same model as
/// `merge_entries`). Note: restating a language in the project resets its
/// `exclude_extensions` — see docs/reference/configuration.md.
fn merge_language_entries(entries: &mut Vec<LanguageConfig>, overrides: Vec<LanguageConfig>) {
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

/// Normalize a language name: trim, lowercase. Shared by the config layer
/// (language-key matching at load/merge) and `aifed::language` (overlay
/// matching), so there is a single definition of "normalized language".
pub fn normalize_language(language: &str) -> String {
    language.trim().to_ascii_lowercase()
}

/// Normalize a file extension: trim, strip a leading dot, lowercase. Shared by
/// the config layer (overlay normalization at load) and `aifed::language`
/// (file-extension resolution), so there is a single definition of "normalized".
pub fn normalize_extension(extension: &str) -> String {
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
    fn loads_lsp_entries_and_workspace_detection() {
        let dir = tempfile::tempdir().unwrap();
        write_file(
            &dir.path().join("Cargo.toml"),
            "[package]\nname='demo'\nversion='0.1.0'\n",
        );

        let global_config = dir.path().join("config.toml");
        write_file(
            &global_config,
            r#"
[[lsp]]
language = "rust"
root_markers = ["Cargo.toml"]
command = "rust-analyzer"
"#,
        );

        let registry = load_registry_from_paths(Some(&global_config), None).unwrap();
        assert_eq!(registry.entries().len(), 1);
        let workspace_languages = registry.detect_languages_for_workspace(dir.path());
        assert_eq!(workspace_languages.len(), 1);
        assert_eq!(workspace_languages[0].language, "rust");
    }

    #[test]
    fn empty_registry_when_no_config() {
        let registry = load_registry_from_paths(None, None).unwrap();
        assert!(registry.entries().is_empty());
        assert!(registry.language_overlays().is_empty());
    }

    #[test]
    fn project_config_loads_without_global() {
        let dir = tempfile::tempdir().unwrap();
        let project_config = dir.path().join("aifed.toml");
        write_file(
            &project_config,
            r#"
[[lsp]]
language = "rust"
root_markers = ["Cargo.toml", "rust-project.json"]
command = "custom-rust-analyzer"
args = ["--stdio"]
"#,
        );

        let registry = load_registry_from_paths(None, Some(&project_config)).unwrap();
        let rust = registry.find_by_language("rust").unwrap();

        assert_eq!(rust.command, "custom-rust-analyzer");
        assert_eq!(rust.args, vec!["--stdio"]);
        assert_eq!(rust.root_markers, vec!["Cargo.toml", "rust-project.json"]);
    }

    #[test]
    fn project_config_declares_config_only_language() {
        // `foo` has no shipped grammar: it appears as a `[[language]]` overlay
        // (so .foo resolves to it) plus an `[[lsp]]` entry. Resolution of
        // .foo -> foo is exercised in crates/aifed/src/language.rs.
        let dir = tempfile::tempdir().unwrap();
        let project_config = dir.path().join("aifed.toml");
        write_file(
            &project_config,
            r#"
[[language]]
language = "foo"
additional_extensions = ["foo"]

[[lsp]]
language = "foo"
root_markers = ["foo.mod"]
command = "foo-lsp"
"#,
        );

        let registry = load_registry_from_paths(None, Some(&project_config)).unwrap();
        let foo = registry.find_by_language("foo").unwrap();
        assert_eq!(foo.command, "foo-lsp");
        let overlay = registry
            .language_overlays()
            .iter()
            .find(|o| o.language == "foo")
            .unwrap();
        assert_eq!(overlay.additional_extensions, vec!["foo"]);
    }

    #[test]
    fn language_overlay_merge_replaces_global() {
        // Project restates `markdown` wholesale, dropping the global exclude.
        let dir = tempfile::tempdir().unwrap();
        let global_config = dir.path().join("global.toml");
        let project_config = dir.path().join("project.toml");
        write_file(
            &global_config,
            r#"
[[language]]
language = "markdown"
exclude_extensions = ["mdx"]
"#,
        );
        write_file(
            &project_config,
            r#"
[[language]]
language = "markdown"
additional_extensions = ["mdown"]
"#,
        );

        let registry =
            load_registry_from_paths(Some(&global_config), Some(&project_config)).unwrap();
        let md = registry
            .language_overlays()
            .iter()
            .find(|o| o.language == "markdown")
            .unwrap();
        assert_eq!(md.additional_extensions, vec!["mdown"]);
        assert!(
            md.exclude_extensions.is_empty(),
            "project replaced global wholesale"
        );
    }

    #[test]
    fn indent_config_parses_and_defaults() {
        // No [indent] table at all -> default assist = true.
        let registry = load_registry_from_paths(None, None).unwrap();
        assert!(registry.indent().assist);

        // Project declares [indent] and per-language indent fields.
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project.toml");
        write_file(
            &project,
            r#"
[indent]
assist = false

[[language]]
language = "rust"
indent_style = "space"
indent_width = 4
"#,
        );
        let registry = load_registry_from_paths(None, Some(&project)).unwrap();
        assert!(!registry.indent().assist);
        let rust = registry
            .language_overlays()
            .iter()
            .find(|o| o.language == "rust")
            .unwrap();
        assert_eq!(rust.indent_style, Some(IndentStyleConfig::Space));
        assert_eq!(rust.indent_width, Some(4));
    }

    #[test]
    fn language_overlay_rejects_zero_space_indent_width() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project.toml");
        write_file(
            &project,
            r#"
[[language]]
language = "rust"
indent_style = "space"
indent_width = 0
"#,
        );
        let err = load_registry_from_paths(None, Some(&project)).unwrap_err();
        assert!(
            err.to_string().contains("indent_width must be at least 1"),
            "{err}"
        );
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
root_markers = ["Cargo.toml"]
command = "global-rust-analyzer"
"#,
        );
        write_file(
            &project_config,
            r#"
[[lsp]]
language = "rust"
root_markers = ["Cargo.toml"]
command = "project-rust-analyzer"
"#,
        );

        let registry =
            load_registry_from_paths(Some(&global_config), Some(&project_config)).unwrap();
        assert_eq!(
            registry.find_by_language("rust").unwrap().command,
            "project-rust-analyzer"
        );
    }

    #[test]
    fn duplicate_lsp_language_in_single_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let project_config = dir.path().join("aifed.toml");
        write_file(
            &project_config,
            r#"
[[lsp]]
language = "rust"
root_markers = ["Cargo.toml"]
command = "rust-analyzer"

[[lsp]]
language = "rust"
root_markers = ["Cargo.toml"]
command = "custom-rust-analyzer"
"#,
        );

        let error = load_registry_from_paths(None, Some(&project_config)).unwrap_err();
        assert!(matches!(error, ConfigError::DuplicateLanguage { .. }));
    }

    #[test]
    fn duplicate_language_overlay_in_single_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let project_config = dir.path().join("aifed.toml");
        write_file(
            &project_config,
            r#"
[[language]]
language = "rust"
additional_extensions = ["rs2"]

[[language]]
language = "rust"
exclude_extensions = ["rs"]
"#,
        );

        let error = load_registry_from_paths(None, Some(&project_config)).unwrap_err();
        assert!(matches!(error, ConfigError::DuplicateLanguage { .. }));
    }
}
