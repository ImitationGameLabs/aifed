//! Go outline extraction via tree-sitter.
//!
//! Drives the generic [`super::walker`] via [`GoSpec`]. Go declarations are
//! syntactically top-level (no symbol is declared inside another), so every
//! item is a leaf (`body: None`). Two containers need [`Spec::expand`]
//! fan-out: `type_declaration` (one or more `type_spec`/`type_alias`) and
//! `var_declaration`/`const_declaration`; a grouped `var (...)` wraps its
//! specs in a `var_spec_list` that must be pierced (consts emit specs directly).

use std::path::Path;

use tree_sitter::{Language, Node};

use super::helpers::{field_text, find_child_by_kind, named_children};
use super::model::{ItemKind, OutlineItem};
use super::spec::{Classified, DocPolicy, Spec};
use super::walker;
use crate::error::Result;

/// Extract a Go source outline. `imports` controls whether `import` items appear.
pub fn extract(source: &str, imports: bool, path: &Path) -> Result<Vec<OutlineItem>> {
    walker::extract(&GoSpec, source, imports, path)
}

/// Per-language spec driving the generic walker for Go.
pub struct GoSpec;

/// Doc attribution for Go: a contiguous preceding `//`/`/* */` block is the
/// Godoc (`doc_prefixes: &[]` attaches every comment), with a blank-line break.
const GO_DOC_POLICY: DocPolicy = DocPolicy {
    attribute_kinds: &[],
    decorator_kinds: &[],
    attach_extras: true,
    doc_prefixes: &[],
    expand_backward: true,
    blank_line_breaks: true,
};

impl Spec for GoSpec {
    fn language(&self) -> Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn grammar_name(&self) -> &'static str {
        "go"
    }

    /// Fan containers into the nodes that carry a name: `type_declaration` -> its
    /// `type_spec`/`type_alias` children; `var_declaration`/`const_declaration` ->
    /// their specs, piercing the grouped `var_spec_list` wrapper.
    fn expand<'a>(&self, node: Node<'a>) -> Vec<Node<'a>> {
        match node.kind() {
            "type_declaration" => named_children(node),
            "var_declaration" | "const_declaration" => flatten_decls(node),
            _ => vec![node],
        }
    }

    fn classify<'a>(&self, node: Node<'a>, source: &str) -> Option<Classified<'a>> {
        let (kind, name, detail) = match node.kind() {
            "function_declaration" => (ItemKind::Function, field_text(node, "name", source), None),
            // A method's name is a `field_identifier` and its receiver sits in a
            // `parameter_list`; surface the receiver type as auxiliary detail.
            "method_declaration" => (
                ItemKind::Function,
                field_text(node, "name", source),
                receiver_type(node, source),
            ),
            "type_alias" => (ItemKind::TypeAlias, field_text(node, "name", source), None),
            // A `type_spec`'s kind depends on its `type` field's resolved child.
            "type_spec" => (type_spec_kind(node), field_text(node, "name", source), None),
            "var_spec" => (ItemKind::Variable, spec_names(node, source), None),
            "const_spec" => (ItemKind::Const, spec_names(node, source), None),
            "import_declaration" => (ItemKind::Imports, import_paths(node, source), None),
            _ => return None,
        };
        if name.is_empty() {
            return None;
        }
        let imports_gated = matches!(kind, ItemKind::Imports);
        Some(Classified { kind, name, body: None, detail, imports_gated })
    }

    fn doc_policy(&self) -> &'static DocPolicy {
        &GO_DOC_POLICY
    }
}

/// Kind of a `type_spec` from its `type` field: struct/interface get their own
/// kind. A defined type (`type N int`, `type Handler func()`) falls through to
/// `TypeAlias`; the model has no separate defined-type kind, so it is reused.
fn type_spec_kind(node: Node<'_>) -> ItemKind {
    match node.child_by_field_name("type").map(|t| t.kind()) {
        Some("struct_type") => ItemKind::Struct,
        Some("interface_type") => ItemKind::Interface,
        _ => ItemKind::TypeAlias,
    }
}

/// Receiver type of a `method_declaration`, e.g. `T` or `*T`. `None` when the
/// receiver or its type can't be found.
fn receiver_type(node: Node<'_>, source: &str) -> Option<String> {
    let receiver = node.child_by_field_name("receiver")?; // parameter_list
    let param = find_child_by_kind(receiver, "parameter_declaration")?;
    let ty = field_text(param, "type", source);
    (!ty.is_empty()).then_some(ty)
}

/// All `name` children of a `var_spec`/`const_spec` joined with `", "` (Go allows
/// `var a, b int`). A single name yields itself.
fn spec_names(node: Node<'_>, source: &str) -> String {
    let mut cursor = node.walk();
    node.children_by_field_name("name", &mut cursor)
        .filter_map(|n| {
            n.utf8_text(source.as_bytes())
                .ok()
                .map(|t| t.trim().to_string())
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Import paths of an `import_declaration` joined with `", "`: `"fmt"` or the
/// grouped `"a", "b"` form. Quotes stripped from each `string_literal` path.
fn import_paths(node: Node<'_>, source: &str) -> String {
    fn collect(node: Node<'_>, source: &str, out: &mut Vec<String>) {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            match child.kind() {
                "import_spec" => out.push(path_text(child, source)),
                "import_spec_list" => collect(child, source, out),
                _ => {}
            }
        }
    }
    let mut paths = Vec::new();
    collect(node, source, &mut paths);
    paths.join(", ")
}

/// Text of an `import_spec`'s `path` field with the surrounding quotes stripped.
fn path_text(spec: Node<'_>, source: &str) -> String {
    field_text(spec, "path", source)
        .trim_matches('"')
        .to_string()
}

/// Flatten a `var_declaration`/`const_declaration` into its `*_spec` children.
/// Only the grouped `var (...)` wraps specs in a `var_spec_list`; a grouped
/// `const (...)` and single `var`/`const` emit `*_spec` children directly.
fn flatten_decls(node: Node<'_>) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    for child in named_children(node) {
        match child.kind() {
            "var_spec_list" => out.extend(named_children(child)),
            _ => out.push(child),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outline::test_support::find;
    use std::path::Path;

    fn extract(src: &str, imports: bool) -> Vec<OutlineItem> {
        super::extract(src, imports, Path::new("test.go")).unwrap()
    }

    #[test]
    fn top_level_kinds_and_names() {
        let src = "package main\n\
                   type Point struct {\n    X int\n\
                   }\n\
                   type Reader interface {\n    Read()\n\
                   }\n\
                   type Num int\n\
                   func add(x int, y int) int {\n    return x + y\n\
                   }\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "Point").kind, ItemKind::Struct);
        assert_eq!(find(&items, "Reader").kind, ItemKind::Interface);
        assert_eq!(find(&items, "Num").kind, ItemKind::TypeAlias);
        assert_eq!(find(&items, "add").kind, ItemKind::Function);
    }

    #[test]
    fn method_receiver_surfaced_as_detail() {
        let src = "package main\n\
                   type T struct {}\n\
                   func (t T) Value() {}\n\
                   func (t *T) Pointer() {}\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "Value").kind, ItemKind::Function);
        assert_eq!(find(&items, "Value").detail.as_deref(), Some("T"));
        assert_eq!(find(&items, "Pointer").detail.as_deref(), Some("*T"));
    }

    #[test]
    fn type_group_fans_out_to_specs() {
        // `type ( A int; B string )` is one type_declaration with two type_specs.
        let src = "package main\n\
                   type (\n    A int\n    B string\n\
                   )\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "A").kind, ItemKind::TypeAlias);
        assert_eq!(find(&items, "B").kind, ItemKind::TypeAlias);
    }

    #[test]
    fn var_group_pierced_and_multiname_joined() {
        // Grouped `var (...)` wraps specs in var_spec_list; `a, b int` is one
        // var_spec with two names.
        let src = "package main\n\
                   var (\n    a, b int\n    c string\n\
                   )\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "a, b").kind, ItemKind::Variable);
        assert_eq!(find(&items, "c").kind, ItemKind::Variable);
    }

    #[test]
    fn const_spec_is_const() {
        let src = "package main\nconst Pi = 3.14\n";
        assert_eq!(find(&extract(src, false), "Pi").kind, ItemKind::Const);
    }

    #[test]
    fn imports_hidden_by_default_shown_with_flag() {
        let src = "package main\nimport \"fmt\"\nfunc main() {}\n";
        assert!(
            extract(src, false)
                .iter()
                .all(|i| i.kind != ItemKind::Imports),
            "import hidden without --imports"
        );
        let with_imports = extract(src, true);
        assert_eq!(
            with_imports
                .iter()
                .find(|i| i.kind == ItemKind::Imports)
                .map(|i| i.name.as_str()),
            Some("fmt")
        );
    }

    #[test]
    fn godoc_extends_start() {
        // The `//` Godoc on line 2 attaches to the func on line 3 -> [2,3].
        let src = "package main\n// Add returns a sum.\nfunc Add() {}\n";
        let items = extract(src, false);
        let add = find(&items, "Add");
        assert_eq!((add.start_line, add.end_line), (2, 3));
    }

    #[test]
    fn truncated_input_does_not_panic() {
        // tree-sitter is fault-tolerant; broken code must not crash the extractor.
        let _ = extract("package main\nfunc foo() {\n", false);
        let _ = extract("type (\n", false);
    }

    #[test]
    fn ranges_are_locator_compatible() {
        let src = "package main\nfunc a() {}\nfunc b() {}\ntype T struct{}\n";
        for item in &extract(src, false) {
            let loc = format!("[{},{}]", item.start_line, item.end_line);
            crate::locator::Locator::parse(&loc).unwrap();
        }
    }
}
