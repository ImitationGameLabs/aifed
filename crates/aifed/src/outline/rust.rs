//! Rust outline extraction via tree-sitter.
//!
//! Walks the `source_file` tree, mapping each top-level (and nested, inside
//! `impl`/`trait`/`mod`/`extern` bodies) item to an [`OutlineItem`] with a
//! 1-based inclusive line range. Ranges are extended back over attached doc
//! comments and outer attributes so `aifed read <FILE> [start,end]` returns the
//! documented item.

use std::path::Path;

use tree_sitter::{Language, Node};

use super::helpers::{field_text, find_child_by_kind};
use super::model::{ItemKind, OutlineItem};
use super::spec::{Classified, DocPolicy, Spec};
use super::walker;
use crate::error::Result;

/// Extract a Rust source outline. `imports` controls whether `use` items appear.
pub fn extract(source: &str, imports: bool, path: &Path) -> Result<Vec<OutlineItem>> {
    walker::extract(&RustSpec, source, imports, path)
}

/// Fold the leading preamble and tile the top-level ranges so the outline's
/// top-level entries cover `[1, total_lines]`. Called by `super::extract` after
/// the raw extraction; the synthetic `file header` over the preamble is added
/// separately by [`super::prepend_file_header`] (shared with markdown).
///
/// - **Fold**: drop the leading run of bodyless declarations (`mod foo;`,
///   `use`) that would otherwise render as single-line noise. An inline
///   `mod foo {}` has a body (`has_body`) and is kept.
/// - **Tile**: rewrite each top-level `end_line` to the next item's
///   `start_line - 1` (last → `total_lines`), so the top-level ranges are
///   adjacent and jointly cover the file. Nested children keep their precise
///   node-end ranges — top-level tiles the file, nested pinpoints (so the
///   top-level cover is a tile, not a global partition: a child's range nests
///   inside its parent's).
///
/// Under normal sibling ordering the next item's `start_line` is always greater
/// than this item's node end — `effective_start_line` pulls a start back only
/// over its *own* preceding doc/attribute siblings, which sit *after* the
/// previous item's node end — so the tiled `end` never inverts. The
/// `max(start_line)` clamp nonetheless guarantees `start <= end` should a parser
/// quirk ever narrow an item.
pub fn tile_top_level(mut items: Vec<OutlineItem>, total_lines: usize) -> Vec<OutlineItem> {
    if items.is_empty() {
        return items;
    }
    // Drop the leading bodyless declarations (the preamble). A bodyless `mod
    // foo;` has no body; an inline `mod foo {}` does (`has_body`), so it stays.
    items = items
        .into_iter()
        .skip_while(|i| {
            matches!(i.kind, ItemKind::Imports)
                || (matches!(i.kind, ItemKind::Module) && !i.has_body)
        })
        .collect();
    if items.is_empty() {
        return items;
    }
    // Tile top-level ends: each ends just before the next begins, the last runs
    // to end of file — adjacency gives a gap-free cover of [1, total_lines].
    let last = items.len() - 1;
    for i in 0..items.len() {
        let end = if i == last { total_lines } else { items[i + 1].start_line.saturating_sub(1) };
        items[i].end_line = end.max(items[i].start_line);
    }
    items
}

/// Per-language spec driving the generic walker for Rust.
pub struct RustSpec;

/// Doc/attribute attribution for Rust: outer `///`/`/**` doc comments and
/// `#[...]` outer attributes, with a blank-line break.
const RUST_DOC_POLICY: DocPolicy = DocPolicy {
    attribute_kinds: &["attribute_item"],
    decorator_kinds: &[],
    attach_extras: true,
    doc_prefixes: &["///", "/**"],
    expand_backward: true,
    blank_line_breaks: true,
};

impl Spec for RustSpec {
    fn language(&self) -> Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn grammar_name(&self) -> &'static str {
        "rust"
    }

    fn classify<'a>(&self, node: Node<'a>, source: &str) -> Option<Classified<'a>> {
        let kind = match node.kind() {
            "function_item" | "function_signature_item" => ItemKind::Function,
            "struct_item" => ItemKind::Struct,
            "enum_item" => ItemKind::Enum,
            "union_item" => ItemKind::Union,
            "trait_item" => ItemKind::Trait,
            "impl_item" => ItemKind::Impl,
            "mod_item" => ItemKind::Module,
            "type_item" | "associated_type" => ItemKind::TypeAlias,
            "const_item" => ItemKind::Const,
            "static_item" => ItemKind::Static,
            "macro_definition" => ItemKind::Macro,
            "foreign_mod_item" => ItemKind::Extern,
            "use_declaration" => ItemKind::Imports,
            // Skip-list: standalone attributes, inner attributes, empty
            // statements, comments, ERROR/MISSING nodes, unrecognized kinds.
            _ => return None,
        };

        let imports_gated = matches!(kind, ItemKind::Imports);

        let name = match kind {
            ItemKind::Impl => impl_name(node, source),
            ItemKind::Extern => extern_name(node, source),
            ItemKind::Imports => field_text(node, "argument", source),
            _ => name_text(node, source),
        };

        // Only containers with a `body` field are recursed into; recording
        // that fact lets `tile_top_level` tell a bodyless `mod foo;` (forward
        // declaration) apart from an inline `mod foo {}` (both may have empty
        // children).
        let body = if matches!(
            kind,
            ItemKind::Impl | ItemKind::Trait | ItemKind::Module | ItemKind::Extern
        ) {
            node.child_by_field_name("body")
        } else {
            None
        };

        Some(Classified { kind, name, body, detail: None, imports_gated })
    }

    fn doc_policy(&self) -> &'static DocPolicy {
        &RUST_DOC_POLICY
    }
}

/// Text of the `name` field (identifier or metavariable), trimmed.
fn name_text(node: Node<'_>, source: &str) -> String {
    field_text(node, "name", source)
}

/// Display name for an `impl` block: `Trait for Type` when a trait is present,
/// otherwise just the inherent `Type`.
fn impl_name(node: Node<'_>, source: &str) -> String {
    let ty = field_text(node, "type", source);
    if let Some(tr) = node.child_by_field_name("trait") {
        let tr_text = tr
            .utf8_text(source.as_bytes())
            .map(|t| t.trim().to_string())
            .unwrap_or_default();
        if !tr_text.is_empty() {
            return format!("{tr_text} for {ty}");
        }
    }
    ty
}

/// Display name for a `extern "ABI" { ... }` block: the ABI string, with the
/// surrounding quotes of the string literal stripped (e.g. `"C"` -> `C`).
fn extern_name(node: Node<'_>, source: &str) -> String {
    find_child_by_kind(node, "extern_modifier")
        .and_then(|em| find_child_by_kind(em, "string_literal"))
        .and_then(|s| s.utf8_text(source.as_bytes()).ok())
        .map(|t| {
            let t = t.trim();
            t.strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
                .map(|inner| inner.to_string())
                .unwrap_or_else(|| t.to_string())
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outline::test_support::find;
    use std::path::Path;
    // `extract` returns RAW items with precise node ranges (pre-tiling): the
    // range assertions below check the raw extractor. The preamble fold and
    // top-level tiling live in `tile_top_level`, tested in the second block
    // below; the full pipeline (tile + `file header`) is tested in `super::super`.

    fn extract(src: &str, imports: bool) -> Vec<OutlineItem> {
        super::extract(src, imports, Path::new("test.rs")).unwrap()
    }

    #[test]
    fn top_level_kinds_and_names() {
        let src = "struct Foo;\nenum Bar { A, B }\nfn baz() {}\nconst N: u32 = 1;\n\
                   static S: u32 = 2;\ntype T = u32;\ntrait Tr {}\nunion U { a: u32 }\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "Foo").kind, ItemKind::Struct);
        assert_eq!(find(&items, "Bar").kind, ItemKind::Enum);
        assert_eq!(find(&items, "baz").kind, ItemKind::Function);
        assert_eq!(find(&items, "N").kind, ItemKind::Const);
        assert_eq!(find(&items, "S").kind, ItemKind::Static);
        assert_eq!(find(&items, "T").kind, ItemKind::TypeAlias);
        assert_eq!(find(&items, "Tr").kind, ItemKind::Trait);
        assert_eq!(find(&items, "U").kind, ItemKind::Union);
    }

    #[test]
    fn function_modifiers_still_named() {
        let items = extract(
            "async fn a() {}\npub fn b() {}\nconst fn c() {}\nunsafe fn d() {}\n",
            false,
        );
        for name in &["a", "b", "c", "d"] {
            assert_eq!(find(&items, name).kind, ItemKind::Function, "{name}");
        }
    }

    #[test]
    fn associated_type_in_trait_is_included() {
        let items = extract(
            "trait Tr {\n    fn t();\n    type Y;\n    const K: u32;\n}\n",
            false,
        );
        let tr = find(&items, "Tr");
        assert_eq!(
            tr.children.iter().find(|c| c.name == "t").map(|c| c.kind),
            Some(ItemKind::Function)
        );
        assert_eq!(
            tr.children.iter().find(|c| c.name == "Y").map(|c| c.kind),
            Some(ItemKind::TypeAlias),
            "associated type Y was dropped"
        );
        assert_eq!(
            tr.children.iter().find(|c| c.name == "K").map(|c| c.kind),
            Some(ItemKind::Const)
        );
    }

    #[test]
    fn impl_recurses_and_names() {
        let items = extract(
            "impl Foo { fn method(&self) {} }\nimpl Tr for Foo { fn t(&self) {} }\n",
            false,
        );
        let impls: Vec<&OutlineItem> = items.iter().filter(|i| i.kind == ItemKind::Impl).collect();
        assert_eq!(impls.len(), 2);
        let inherent = impls.iter().find(|i| i.name == "Foo").unwrap();
        assert_eq!(inherent.detail, None);
        assert_eq!(
            inherent
                .children
                .iter()
                .find(|c| c.name == "method")
                .map(|c| c.kind),
            Some(ItemKind::Function)
        );
        let trait_impl = impls.iter().find(|i| i.name == "Tr for Foo").unwrap();
        assert_eq!(
            trait_impl
                .children
                .iter()
                .find(|c| c.name == "t")
                .map(|c| c.kind),
            Some(ItemKind::Function)
        );
    }

    #[test]
    fn doc_comment_extends_start() {
        // line 1: /// doc, line 2: fn -> [1,2] (doc included)
        let items = extract("/// Docs for foo.\nfn foo() {}\n", false);
        let foo = find(&items, "foo");
        assert_eq!((foo.start_line, foo.end_line), (1, 2));
    }

    #[test]
    fn blank_line_breaks_doc_attachment() {
        // line 1: doc, line 2: blank, line 3: fn -> start at fn line
        let items = extract("/// Dangling.\n\nfn foo() {}\n", false);
        let foo = find(&items, "foo");
        assert_eq!((foo.start_line, foo.end_line), (3, 3));
    }

    #[test]
    fn inner_doc_not_attached() {
        // `//!` belongs to the module, not the following fn.
        let items = extract("//! Module doc.\nfn foo() {}\n", false);
        let foo = find(&items, "foo");
        assert_eq!(foo.start_line, 2);
    }

    #[test]
    fn attribute_extends_start() {
        let items = extract("#[derive(Debug)]\nfn foo() {}\n", false);
        let foo = find(&items, "foo");
        assert_eq!((foo.start_line, foo.end_line), (1, 2));
    }

    #[test]
    fn mod_without_body_is_leaf() {
        let items = extract("mod foo;\n", false);
        let foo = find(&items, "foo");
        assert_eq!(foo.kind, ItemKind::Module);
        assert!(foo.children.is_empty());
    }

    #[test]
    fn mod_with_body_recurses() {
        let items = extract("mod m {\n    fn inner() {}\n}\n", false);
        let m = find(&items, "m");
        assert_eq!(
            m.children
                .iter()
                .find(|c| c.name == "inner")
                .map(|c| c.kind),
            Some(ItemKind::Function)
        );
    }

    #[test]
    fn use_hidden_by_default_shown_with_flag() {
        assert!(
            extract("use std::io;\nfn foo() {}\n", false)
                .iter()
                .all(|i| i.kind != ItemKind::Imports)
        );
        let with_imports = extract("use std::io;\nfn foo() {}\n", true);
        assert_eq!(
            with_imports
                .iter()
                .find(|i| i.kind == ItemKind::Imports)
                .map(|i| i.name.as_str()),
            Some("std::io")
        );
    }

    #[test]
    fn macro_rules_recognized() {
        assert_eq!(
            find(&extract("macro_rules! m {\n    () => {};\n}\n", false), "m").kind,
            ItemKind::Macro
        );
    }

    #[test]
    fn extern_block_recurses_with_unquoted_abi() {
        let items = extract("extern \"C\" {\n    fn f();\n}\n", false);
        let ext = items.iter().find(|i| i.kind == ItemKind::Extern).unwrap();
        assert_eq!(ext.name, "C", "extern ABI should be unquoted");
        assert_eq!(
            ext.children.iter().find(|c| c.name == "f").map(|c| c.kind),
            Some(ItemKind::Function)
        );
    }

    #[test]
    fn truncated_input_does_not_panic() {
        // tree-sitter is fault-tolerant; broken code must not crash the extractor.
        let _ = extract("fn foo() {\n    let x = 1;\n", false);
        let _ = extract("struct Foo {\n    x: i32,\n", false);
        let _ = extract("pub macro n {}\n", false);
    }

    #[test]
    fn ranges_are_locator_compatible() {
        let items = extract("fn a() {}\nfn b() {}\nimpl C { fn d() {} }\n", false);
        for item in &items {
            let loc = format!("[{},{}]", item.start_line, item.end_line);
            crate::locator::Locator::parse(&loc).unwrap();
            for child in &item.children {
                let loc = format!("[{},{}]", child.start_line, child.end_line);
                crate::locator::Locator::parse(&loc).unwrap();
            }
        }
    }

    // --- tile_top_level: preamble fold + top-level tiling ---

    fn tiled(src: &str, imports: bool) -> Vec<OutlineItem> {
        tile_top_level(extract(src, imports), src.lines().count())
    }

    #[test]
    fn tile_folds_leading_bodyless_mods() {
        // `mod a;` / `mod b;` are bodyless forward declarations → dropped; the
        // surviving definition tiles to end of file.
        let items = tiled("mod a;\nmod b;\nfn main() {}\n", false);
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].name, "main");
        assert!(items.iter().all(|i| i.kind != ItemKind::Module));
        assert_eq!(items[0].end_line, 3);
    }

    #[test]
    fn tile_keeps_inline_module_with_body() {
        // `mod m { .. }` has a body → kept (not folded into a preamble).
        let items = tiled("mod m {\n    fn inner() {}\n}\nfn main() {}\n", false);
        let m = items
            .iter()
            .find(|i| i.name == "m")
            .expect("inline mod kept");
        assert!(m.has_body);
        assert_eq!(
            m.children
                .iter()
                .find(|c| c.name == "inner")
                .map(|c| c.kind),
            Some(ItemKind::Function)
        );
    }

    #[test]
    fn tile_keeps_empty_inline_module() {
        // An inline `mod foo {}` with an EMPTY body still has a body field →
        // `has_body` is true, so it must NOT be folded like `mod foo;`. This is
        // the exact boundary `has_body` exists to handle.
        let items = tiled("mod foo {}\nfn main() {}\n", false);
        let foo = items
            .iter()
            .find(|i| i.name == "foo")
            .expect("empty inline mod kept");
        assert_eq!(foo.kind, ItemKind::Module);
        assert!(foo.has_body);
    }

    #[test]
    fn tile_top_level_ranges_are_adjacent_and_cover() {
        let items = tiled("fn a() {}\n\nfn b() {}\nfn c() {}\n", false);
        let total = 4; // "fn a() {}", "", "fn b() {}", "fn c() {}"
        // first starts at 1, each end abuts the next start, last reaches total.
        assert_eq!(items[0].start_line, 1);
        for pair in items.windows(2) {
            assert_eq!(pair[0].end_line + 1, pair[1].start_line, "{items:?}");
        }
        assert_eq!(items.last().unwrap().end_line, total);
    }

    #[test]
    fn tile_does_not_invert_on_doc_attribution() {
        // Doc attribution pulls each start back over its own `///`, but never
        // past the previous item's end — so tiled ends stay start <= end <= next.
        let items = tiled("/// d\nfn a() {}\n/// e\nfn b() {}\n", false);
        assert_eq!(items.len(), 2);
        assert!(items[0].end_line >= items[0].start_line);
        assert!(items[0].end_line < items[1].start_line);
    }

    #[test]
    fn tile_ranges_are_locator_compatible() {
        let items = tiled("mod a;\nfn x() {}\nimpl C { fn y() {} }\n", false);
        for item in &items {
            let loc = format!("[{},{}]", item.start_line, item.end_line);
            crate::locator::Locator::parse(&loc).unwrap();
        }
    }
}
