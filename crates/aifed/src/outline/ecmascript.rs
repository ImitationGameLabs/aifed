//! Shared classification for the ECMAScript family (JavaScript, TypeScript).
//!
//! Both grammars share the core declaration kinds (functions, classes, methods,
//! variable bindings, imports, exports). Each language's `Spec` is a thin
//! wrapper that supplies its tree-sitter grammar + name and delegates
//! [`expand`]/[`classify`]/[`ECMASCRIPT_DOC_POLICY`] here. TypeScript adds
//! interface/enum/abstract-class/type-alias, classified here too (inert under JS).
//!
//! Name extraction is deliberately **field-based** (`field_text(node, "name",
//! …)`): JavaScript uses `identifier` and TypeScript uses `type_identifier` for
//! the same syntactic position, so reading the `name` field works for both.

use tree_sitter::Node;

use super::helpers::{field_text, named_children};
use super::model::ItemKind;
use super::spec::{Classified, DocPolicy};

/// Doc attribution for the ECMAScript family: a contiguous preceding comment
/// block (`//` or `/** */`) is the doc (`doc_prefixes: &[]` attaches every
/// comment), with a blank-line break. JSDoc needs no special prefix here since
/// any directly-preceding comment is treated as documentation.
pub(super) const ECMASCRIPT_DOC_POLICY: DocPolicy = DocPolicy {
    attribute_kinds: &[],
    decorator_kinds: &[],
    attach_extras: true,
    doc_prefixes: &[],
    expand_backward: true,
    blank_line_breaks: true,
};

/// Fan containers into the nodes that carry a name. An `export_statement`
/// wraps the exported declaration (unwrap it, then expand that too, so
/// `export const a = 1, b = 2` reaches each declarator); a
/// `variable_declaration`/`lexical_declaration` fans out to its
/// `variable_declarator` children.
pub(super) fn expand<'a>(node: Node<'a>) -> Vec<Node<'a>> {
    match node.kind() {
        // Re-expand: an export can wrap another container (e.g. an exported
        // lexical_declaration), so unwrap recursively to reach each declarator.
        "export_statement" => named_children(node).into_iter().flat_map(expand).collect(),
        "variable_declaration" | "lexical_declaration" => named_children(node)
            .into_iter()
            .filter(|c| c.kind() == "variable_declarator")
            .collect(),
        _ => vec![node],
    }
}

/// Classify one (already-expanded) ECMAScript node, or `None` to skip it.
///
/// Anonymous `export default function`/`class` (an expression, not a
/// declaration) has no name and is skipped; a named `export default` is kept.
pub(super) fn classify<'a>(node: Node<'a>, source: &str) -> Option<Classified<'a>> {
    let (kind, name, body) = match node.kind() {
        "function_declaration" | "generator_function_declaration" => (
            ItemKind::Function,
            field_text(node, "name", source),
            node.child_by_field_name("body"),
        ),
        "class_declaration" => (
            ItemKind::Class,
            field_text(node, "name", source),
            node.child_by_field_name("body"),
        ),
        "method_definition" => (
            ItemKind::Function,
            field_text(node, "name", source),
            node.child_by_field_name("body"),
        ),
        // A declarator is a function when its value is an arrow/function
        // expression (a "named arrow function"); otherwise it is a variable.
        "variable_declarator" => variable_declarator(node, source),
        "import_statement" => (ItemKind::Imports, import_source(node, source), None),
        // TypeScript-only kinds (inert under the JavaScript grammar).
        "interface_declaration" => (ItemKind::Interface, field_text(node, "name", source), None),
        "enum_declaration" => (ItemKind::Enum, field_text(node, "name", source), None),
        "abstract_class_declaration" => (
            ItemKind::Class,
            field_text(node, "name", source),
            node.child_by_field_name("body"),
        ),
        "type_alias_declaration" => (ItemKind::TypeAlias, field_text(node, "name", source), None),
        _ => return None,
    };
    if name.is_empty() {
        return None;
    }
    let imports_gated = matches!(kind, ItemKind::Imports);
    Some(Classified { kind, name, body, detail: None, imports_gated })
}

/// Split a `variable_declarator` into kind/name/body: a function-valued binding
/// (`const f = () => {}`) is a Function whose value's body the walker recurses
/// into; everything else (`const N = 1`, `let x;`) is a Variable leaf.
fn variable_declarator<'a>(node: Node<'a>, source: &str) -> (ItemKind, String, Option<Node<'a>>) {
    let name = field_text(node, "name", source);
    let Some(value) = node.child_by_field_name("value") else {
        return (ItemKind::Variable, name, None);
    };
    if matches!(value.kind(), "arrow_function" | "function_expression") {
        (ItemKind::Function, name, value.child_by_field_name("body"))
    } else {
        (ItemKind::Variable, name, None)
    }
}

/// Module source of an `import` statement (the `source` string literal), with
/// surrounding quotes stripped (e.g. `'mod'` -> `mod`).
fn import_source(node: Node<'_>, source: &str) -> String {
    field_text(node, "source", source)
        .trim_matches(|c| c == '\'' || c == '"')
        .to_string()
}
