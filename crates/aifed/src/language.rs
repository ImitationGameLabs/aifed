//! Single source of truth for file-extension → language resolution, shared by
//! `outline` (pick a grammar) and the `lsp` CLI (pick a server).
//!
//! Grammar-default extensions are a compile-time table
//! ([`GRAMMAR_DEFAULTS`]); `[[language]]` config overlays layer on top:
//!
//! ```text
//! effective = (grammar_defaults ∪ additional_extensions) − exclude_extensions
//! ```
//!
//! Both consumers resolve a file to a language *name* here, then each maps that
//! name to its own concern — `outline` to a tree-sitter grammar, `lsp` to a
//! server entry. The resolver is grammar-agnostic (no tree-sitter imports), so
//! it sits at the crate root rather than under `outline`.

use std::path::Path;

use aifed_common::{LanguageConfig, normalize_extension, normalize_language};

/// Languages aifed ships a grammar for, with their canonical default extensions.
///
/// Adding a language is three coordinated steps: a row here, a dispatch arm in
/// `outline::extract`, and the grammar crate dependency. Tests assert every row
/// has a matching dispatch arm and that [`LanguageResolver::supported_extensions`]
/// equals exactly the union of these rows, so the three can't silently drift.
pub const GRAMMAR_DEFAULTS: &[(&str, &[&str])] = &[
    ("rust", &["rs"]),
    ("markdown", &["md", "markdown", "mdx"]),
    ("go", &["go"]),
    ("python", &["py"]),
    ("javascript", &["js", "mjs", "cjs"]),
    ("typescript", &["ts", "tsx"]),
    ("c", &["c", "h"]),
    ("cpp", &["cpp", "cc", "cxx", "hpp", "hh", "hxx"]),
];

/// A language resolved to its effective extension set.
#[derive(Debug)]
struct ResolvedLanguage {
    language: String,
    extensions: Vec<String>,
    /// Part of a shipped grammar ([`GRAMMAR_DEFAULTS`]) vs. a config-only
    /// language (no outline grammar). Only grammar languages appear in
    /// `supported_extensions`.
    has_grammar: bool,
}

/// Resolved extension→language table, ready to answer [`Self::detect`].
///
/// Grammar-default languages always participate; `[[language]]` overlays
/// augment/shrink them or introduce config-only languages. Construction reuses
/// [`aifed_common::normalize_extension`] so "normalized extension" has one
/// definition across the config layer and the resolver.
#[derive(Debug)]
pub struct LanguageResolver {
    // Grammar languages first (in GRAMMAR_DEFAULTS order), then config-only
    // languages in overlay order. detect() scans in this order, so a grammar
    // default wins over any overlay claim on the same extension.
    languages: Vec<ResolvedLanguage>,
}

impl LanguageResolver {
    /// Grammar defaults only — the zero-config path (e.g. `outline` with no
    /// config, or a corrupt config that fell back).
    pub fn from_defaults() -> Self {
        let languages = GRAMMAR_DEFAULTS
            .iter()
            .map(|(language, exts)| ResolvedLanguage {
                language: (*language).to_string(),
                extensions: exts.iter().map(|e| normalize_extension(e)).collect(),
                has_grammar: true,
            })
            .collect();
        Self { languages }
    }

    /// Grammar defaults with `[[language]]` overlays applied. Overlays are
    /// expected pre-normalized and pre-merged (global then project), as produced
    /// by the config loader; `apply_overlay` normalizes again defensively.
    pub fn with_overlays(overlays: &[LanguageConfig]) -> Self {
        let mut languages: Vec<ResolvedLanguage> = GRAMMAR_DEFAULTS
            .iter()
            .map(|(language, exts)| {
                let defaults: Vec<String> = exts.iter().map(|e| normalize_extension(e)).collect();
                let overlay = overlays
                    .iter()
                    .find(|o| normalize_language(&o.language) == *language);
                let extensions = match overlay {
                    Some(o) => {
                        apply_overlay(&defaults, &o.additional_extensions, &o.exclude_extensions)
                    }
                    None => defaults,
                };
                ResolvedLanguage {
                    language: (*language).to_string(),
                    extensions,
                    has_grammar: true,
                }
            })
            .collect();

        // Config-only languages (no grammar row): effective = additional − exclude.
        for overlay in overlays {
            let is_grammar = GRAMMAR_DEFAULTS
                .iter()
                .any(|(lang, _)| lang == &normalize_language(&overlay.language));
            if !is_grammar {
                languages.push(ResolvedLanguage {
                    language: overlay.language.clone(),
                    extensions: apply_overlay(
                        &[],
                        &overlay.additional_extensions,
                        &overlay.exclude_extensions,
                    ),
                    has_grammar: false,
                });
            }
        }

        Self { languages }
    }

    /// Resolve a file's extension to a language name. Case-insensitive.
    ///
    /// Grammar languages are scanned before config-only ones, so a grammar
    /// default (e.g. `rs`→rust) can only be displaced by `exclude`-ing it from
    /// its owner — never by another language claiming the same extension. Among
    /// config-only languages, first-declared (global then project, file order)
    /// wins.
    pub fn detect(&self, file: &Path) -> Option<&str> {
        let raw = file.extension()?.to_str()?;
        let ext = normalize_extension(raw);
        if ext.is_empty() {
            return None;
        }
        self.languages
            .iter()
            .find(|lang| lang.extensions.contains(&ext))
            .map(|lang| lang.language.as_str())
    }

    /// Effective extensions of grammar languages only, each prefixed with `.`
    /// and sorted lexicographically — the single source for the outline
    /// "supported: ..." message. Config-only languages are excluded (outline
    /// can't outline them).
    pub fn supported_extensions(&self) -> Vec<String> {
        let mut exts: Vec<String> = self
            .languages
            .iter()
            .filter(|lang| lang.has_grammar)
            .flat_map(|lang| lang.extensions.iter().cloned())
            .map(|e| format!(".{e}"))
            .collect();
        exts.sort();
        exts.dedup();
        exts
    }
}

/// `effective = (defaults ∪ additional) − exclude`. Normalization is idempotent;
/// this is defensive in case a future caller bypasses the config loader.
fn apply_overlay(defaults: &[String], additional: &[String], exclude: &[String]) -> Vec<String> {
    let mut out: Vec<String> = defaults.to_vec();
    for ext in additional {
        let normalized = normalize_extension(ext);
        if !normalized.is_empty() && !out.contains(&normalized) {
            out.push(normalized);
        }
    }
    let excluded: Vec<String> = exclude.iter().map(|e| normalize_extension(e)).collect();
    out.retain(|e| !excluded.contains(e));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlay(language: &str, additional: &[&str], exclude: &[&str]) -> LanguageConfig {
        LanguageConfig {
            language: language.to_string(),
            additional_extensions: additional.iter().map(|s| s.to_string()).collect(),
            exclude_extensions: exclude.iter().map(|s| s.to_string()).collect(),
            indent_assist: None,
            indent_style: None,
            indent_width: None,
        }
    }

    #[test]
    fn from_defaults_resolves_rust_and_markdown() {
        let reg = LanguageResolver::from_defaults();
        assert_eq!(reg.detect(Path::new("a.rs")), Some("rust"));
        assert_eq!(reg.detect(Path::new("a.md")), Some("markdown"));
        assert_eq!(reg.detect(Path::new("a.mdx")), Some("markdown"));
        assert_eq!(reg.detect(Path::new("a.markdown")), Some("markdown"));
    }

    #[test]
    fn from_defaults_supported_extensions_sorted() {
        let reg = LanguageResolver::from_defaults();
        // Exactly the GRAMMAR_DEFAULTS union, dotted, lexicographically sorted
        // by bytes (`.markdown` precedes `.md` because 'a' < 'd' at index 2).
        assert_eq!(
            reg.supported_extensions(),
            vec![
                ".c".into(),
                ".cc".into(),
                ".cjs".into(),
                ".cpp".into(),
                ".cxx".into(),
                ".go".to_string(),
                ".h".into(),
                ".hh".into(),
                ".hpp".into(),
                ".hxx".into(),
                ".js".into(),
                ".markdown".to_string(),
                ".md".into(),
                ".mdx".into(),
                ".mjs".into(),
                ".py".into(),
                ".rs".into(),
                ".ts".into(),
                ".tsx".into(),
            ]
        );
    }

    #[test]
    fn overlay_adds_extension() {
        let reg = LanguageResolver::with_overlays(&[overlay("markdown", &["mdown"], &[])]);
        assert_eq!(reg.detect(Path::new("a.mdown")), Some("markdown"));
        // Defaults still resolve.
        assert_eq!(reg.detect(Path::new("a.md")), Some("markdown"));
    }

    #[test]
    fn overlay_exclude_removes_default() {
        let reg = LanguageResolver::with_overlays(&[overlay("rust", &[], &["rs"])]);
        // `rs` is excluded from its only owner → nothing claims it.
        assert_eq!(reg.detect(Path::new("a.rs")), None);
        assert!(reg.supported_extensions().iter().all(|e| e != ".rs"));
    }

    #[test]
    fn grammar_default_wins_over_overlay_claim() {
        // A config-only language tries to claim `rs`; rust still owns it.
        let reg = LanguageResolver::with_overlays(&[overlay("other", &["rs"], &[])]);
        assert_eq!(reg.detect(Path::new("a.rs")), Some("rust"));
    }

    #[test]
    fn config_only_language_resolves_but_is_not_supported() {
        let reg = LanguageResolver::with_overlays(&[overlay("ruby", &["rb"], &[])]);
        assert_eq!(reg.detect(Path::new("a.rb")), Some("ruby"));
        // Not a grammar language → absent from the supported list.
        assert!(reg.supported_extensions().iter().all(|e| e != ".rb"));
    }

    #[test]
    fn first_declared_wins_for_config_only_overlap() {
        let reg = LanguageResolver::with_overlays(&[
            overlay("alpha", &["foo"], &[]),
            overlay("beta", &["foo"], &[]),
        ]);
        assert_eq!(reg.detect(Path::new("a.foo")), Some("alpha"));
    }

    #[test]
    fn case_insensitive_and_dotted_input() {
        let reg = LanguageResolver::from_defaults();
        assert_eq!(reg.detect(Path::new("A.RS")), Some("rust"));
        assert_eq!(reg.detect(Path::new("a.MD")), Some("markdown"));
        // A leading dot in an overlay extension is stripped by normalize_extension.
        let reg2 = LanguageResolver::with_overlays(&[overlay("markdown", &[".mdown"], &[])]);
        assert_eq!(reg2.detect(Path::new("a.mdown")), Some("markdown"));
    }

    #[test]
    fn no_extension_and_unknown_return_none() {
        let reg = LanguageResolver::from_defaults();
        assert_eq!(reg.detect(Path::new("Makefile")), None);
        assert_eq!(reg.detect(Path::new("a.xyz")), None);
    }
}
