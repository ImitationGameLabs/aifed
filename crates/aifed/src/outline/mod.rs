//! Structural outline extraction for `aifed outline`.
//!
//! Resolves a file to a language via the shared [`crate::language`] registry
//! (grammar defaults in code, plus `[[language]]` config overlays), then
//! dispatches to a tree-sitter grammar. Daemon-free: with no config (or a
//! corrupt one that the caller fell back from) it uses grammar defaults, so it
//! still works out of the box. Markdown is a first-class outline target even
//! though it has no LSP.

mod c;
mod cpp;
mod ecmascript;
mod go;
mod helpers;
mod javascript;
mod markdown;
mod model;
mod python;
mod render;
mod rust;
mod spec;
#[cfg(test)]
mod test_support;
mod typescript;
mod walker;

pub use model::Outline;
// Re-exported so `output::format_outline` can render without owning the logic.
use model::{ItemKind, OutlineItem};
pub(crate) use render::render_text;

use std::path::Path;

use crate::error::{Error, Result};
use crate::language::LanguageResolver;

/// Extract the outline of `source` (read from `path`), resolving the language
/// via `registry` and dispatching to a tree-sitter grammar. `imports` controls
/// whether Rust `use` items appear. Callers pass a registry built from config
/// overlays (`LanguageResolver::with_overlays`) or, for the zero-config path,
/// `LanguageResolver::from_defaults()`.
pub fn extract(
    path: &Path,
    source: &str,
    imports: bool,
    registry: &LanguageResolver,
) -> Result<Outline> {
    let total_lines = source.lines().count();
    // `Outline.language` is `&'static str`, so we match on the resolved name and
    // produce static literals in the grammar arms; a config-only language (no
    // shipped grammar) hits `Some(other)` and errors cleanly.
    let (language, items) = match registry.detect(path) {
        Some("rust") => (
            "rust",
            prepend_file_header(
                rust::tile_top_level(rust::extract(source, imports, path)?, total_lines),
                total_lines,
            ),
        ),
        Some("markdown") => (
            "markdown",
            prepend_file_header(markdown::extract(source, path)?, total_lines),
        ),
        Some("go") => (
            "go",
            prepend_file_header(go::extract(source, imports, path)?, total_lines),
        ),
        Some("python") => (
            "python",
            prepend_file_header(python::extract(source, imports, path)?, total_lines),
        ),
        Some("javascript") => (
            "javascript",
            prepend_file_header(javascript::extract(source, imports, path)?, total_lines),
        ),
        Some("typescript") => (
            "typescript",
            prepend_file_header(typescript::extract(source, imports, path)?, total_lines),
        ),
        Some("c") => (
            "c",
            prepend_file_header(c::extract(source, imports, path)?, total_lines),
        ),
        Some("cpp") => (
            "cpp",
            prepend_file_header(cpp::extract(source, imports, path)?, total_lines),
        ),
        Some(other) => {
            return Err(Error::OutlineUnsupported {
                path: crate::file::to_absolute(path),
                reason: format!(
                    "no outline grammar for language '{other}' — use `aifed read` to view the file directly"
                ),
            });
        }
        None => {
            return Err(Error::OutlineUnsupported {
                path: crate::file::to_absolute(path),
                reason: unsupported_reason(path, registry),
            });
        }
    };
    Ok(Outline { path: path.display().to_string(), language, total_lines, items })
}

/// Build the "supported: ..." message from the registry so the extension list
/// is derived, not re-typed (it was previously hardcoded in three places, one
/// already stale — missing `.mdx`).
fn unsupported_reason(path: &Path, registry: &LanguageResolver) -> String {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let head = if ext.is_empty() {
        "file has no extension".to_string()
    } else {
        format!("no outline grammar for '.{ext}'")
    };
    format!(
        "{head} (supported: {}) — use `aifed read` to view the file directly",
        registry.supported_extensions().join(", ")
    )
}

/// Prepend a synthetic `file header` region when content precedes the first
/// symbol (leading `mod`/`use`, inner docs, comments) so the outline's
/// top-level ranges tile `[1, total_lines]`. The region covers
/// `[1, first.start - 1]`, or `[1, total_lines]` when there are no symbols.
/// Shared by both extractors.
fn prepend_file_header(mut items: Vec<OutlineItem>, total_lines: usize) -> Vec<OutlineItem> {
    let end = match items.first() {
        Some(first) if first.start_line > 1 => first.start_line - 1,
        Some(_) => return items,
        None if total_lines > 0 => total_lines,
        None => return items,
    };
    items.insert(
        0,
        OutlineItem {
            kind: ItemKind::FileHeader,
            name: "file header".to_string(),
            start_line: 1,
            end_line: end,
            has_body: false,
            level: None,
            detail: None,
            children: Vec::new(),
        },
    );
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::GRAMMAR_DEFAULTS;

    fn registry() -> LanguageResolver {
        LanguageResolver::from_defaults()
    }

    #[test]
    fn dispatches_rust_and_markdown() {
        let rust = extract(Path::new("a.rs"), "fn main() {}\n", false, &registry()).unwrap();
        assert_eq!(rust.language, "rust");
        assert_eq!(rust.items.len(), 1);
        let md = extract(Path::new("a.md"), "# T\n", false, &registry()).unwrap();
        assert_eq!(md.language, "markdown");
        assert_eq!(md.items.len(), 1);
        // markdown alias + mdx
        assert_eq!(
            extract(Path::new("a.markdown"), "# T\n", false, &registry())
                .unwrap()
                .language,
            "markdown"
        );
        assert_eq!(
            extract(Path::new("a.mdx"), "# T\n", false, &registry())
                .unwrap()
                .language,
            "markdown"
        );
    }

    #[test]
    fn every_grammar_default_has_a_dispatch_arm() {
        // Iterating GRAMMAR_DEFAULTS (not a separate list) keeps the table and
        // this test in sync: a missing minimal source panics, and a missing
        // dispatch arm makes extract error with "no outline grammar for 'X'".
        let minimal_source: &[(&str, &str)] = &[
            ("rust", "fn a() {}\n"),
            ("markdown", "# T\n"),
            ("go", "func a() {}\n"),
            ("python", "def a():\n    pass\n"),
            ("javascript", "function a() {}\n"),
            ("typescript", "function a(): void {}\n"),
            ("c", "void a(void) {}\n"),
            ("cpp", "void a() {}\n"),
        ];
        let reg = registry();
        for (lang, exts) in GRAMMAR_DEFAULTS {
            let src = minimal_source
                .iter()
                .find(|(l, _)| l == lang)
                .unwrap_or_else(|| panic!("missing minimal source for grammar language '{lang}'"))
                .1;
            let ext = exts[0];
            let out = extract(Path::new(&format!("file.{ext}")), src, false, &reg);
            assert!(
                out.is_ok(),
                "grammar language '{lang}' (.{ext}) resolved to no dispatch arm: {:?}",
                out.err()
            );
        }
    }

    #[test]
    fn unknown_extension_errors() {
        let err = extract(Path::new("a.toml"), "x = 1\n", false, &registry()).unwrap_err();
        assert!(err.to_string().contains("no outline grammar"), "{}", err);
        // The supported list is derived from the registry and includes .mdx
        // (regression: the old args.rs help omitted it).
        assert!(err.to_string().contains("supported:"), "{}", err);
        assert!(err.to_string().contains(".mdx"), "{}", err);
    }

    #[test]
    fn no_extension_errors() {
        let err = extract(Path::new("Makefile"), "all:\n", false, &registry()).unwrap_err();
        assert!(err.to_string().contains("no extension"), "{}", err);
    }

    #[test]
    fn empty_source_yields_empty_items() {
        let out = extract(Path::new("a.rs"), "", false, &registry()).unwrap();
        assert!(out.items.is_empty());
    }

    // --- full pipeline: tile_top_level + file-header prepend ---

    #[test]
    fn rust_preamble_becomes_file_header_and_ranges_tile() {
        let out = extract(
            Path::new("a.rs"),
            "mod a;\nmod b;\nfn main() {}\n",
            false,
            &registry(),
        )
        .unwrap();
        // file header over [1,2], then `fn main` at line 3 tiling to end of file.
        assert_eq!(out.items.len(), 2);
        assert_eq!(out.items[0].kind, ItemKind::FileHeader);
        assert_eq!((out.items[0].start_line, out.items[0].end_line), (1, 2));
        assert_eq!(out.items[1].name, "main");
        // top-level ranges tile [1, total_lines].
        assert_eq!(out.items[0].start_line, 1);
        assert_eq!(out.items[0].end_line + 1, out.items[1].start_line);
        assert_eq!(out.items.last().unwrap().end_line, out.total_lines);
    }

    #[test]
    fn no_file_header_when_file_opens_with_definition() {
        let out = extract(Path::new("a.rs"), "fn main() {}\n", false, &registry()).unwrap();
        assert!(out.items.iter().all(|i| i.kind != ItemKind::FileHeader));
        assert_eq!(out.items.len(), 1);
    }

    #[test]
    fn header_only_file_when_just_declarations() {
        let out = extract(Path::new("a.rs"), "mod a;\nmod b;\n", false, &registry()).unwrap();
        assert_eq!(out.items.len(), 1);
        assert_eq!(out.items[0].kind, ItemKind::FileHeader);
        assert_eq!(out.total_lines, 2);
        assert_eq!(out.items[0].end_line, out.total_lines);
    }

    #[test]
    fn markdown_prepends_header_for_pre_heading_content() {
        let out = extract(Path::new("a.md"), "intro\n# Title\n", false, &registry()).unwrap();
        // first heading at line 2 → file header over [1,1], then the heading.
        assert_eq!(out.items[0].kind, ItemKind::FileHeader);
        assert_eq!((out.items[0].start_line, out.items[0].end_line), (1, 1));
        assert_eq!(out.items[1].kind, ItemKind::Heading);
    }

    #[test]
    fn markdown_no_header_when_heading_at_line_one() {
        let out = extract(Path::new("a.md"), "# Title\nbody\n", false, &registry()).unwrap();
        assert!(out.items.iter().all(|i| i.kind != ItemKind::FileHeader));
    }

    #[test]
    fn total_lines_and_cover_invariant_hold_across_trailing_newlines() {
        // total_lines must match the 1-based rows tree-sitter emits, and the
        // last top-level range must reach it, regardless of trailing newlines.
        // The preamble fixture also exercises the header + tiled-last case.
        for src in [
            "fn a() {}\n",
            "fn a() {}\nfn b() {}\n",
            "fn a() {}\n\nfn b() {}\n",
            "fn a() {}",
            "mod m;\nfn a() {}\n",
        ] {
            let out = extract(Path::new("a.rs"), src, false, &registry()).unwrap();
            assert_eq!(out.total_lines, src.lines().count(), "src={src:?}");
            assert_eq!(
                out.items.last().unwrap().end_line,
                out.total_lines,
                "src={src:?}"
            );
        }
    }

    #[test]
    fn imports_surfaces_mid_file_use_while_leading_folds() {
        // `--imports` keeps the mid-file `use b;` but the leading `use a;` still
        // folds into the file-header region — and nothing overlaps that region.
        let out = extract(
            Path::new("a.rs"),
            "use a;\nfn first() {}\nuse b;\nfn second() {}\n",
            true,
            &registry(),
        )
        .unwrap();
        assert_eq!(out.items[0].kind, ItemKind::FileHeader);
        assert_eq!((out.items[0].start_line, out.items[0].end_line), (1, 1));
        assert!(
            out.items.iter().all(|i| i.name != "a"),
            "leading use folded away"
        );
        let use_b = out
            .items
            .iter()
            .find(|i| i.name == "b")
            .expect("mid-file use kept");
        assert_eq!(use_b.kind, ItemKind::Imports);
        // no top-level range overlaps the header region.
        for item in &out.items[1..] {
            assert!(item.start_line > out.items[0].end_line, "{item:?}");
        }
    }

    #[test]
    fn pipeline_ranges_are_locator_compatible() {
        let out = extract(
            Path::new("a.rs"),
            "mod a;\nfn x() {}\nimpl C { fn y() {} }\n",
            false,
            &registry(),
        )
        .unwrap();
        for item in &out.items {
            let loc = format!("[{},{}]", item.start_line, item.end_line);
            crate::locator::Locator::parse(&loc).unwrap();
        }
    }
}
