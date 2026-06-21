//! Shared tree-sitter node helpers for language [`super::spec::Spec`] implementations.

use tree_sitter::Node;

/// Text of a named field, trimmed. Empty when the field is absent.
pub(super) fn field_text(node: Node<'_>, field: &str, source: &str) -> String {
    node.child_by_field_name(field)
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .map(|t| t.trim().to_string())
        .unwrap_or_default()
}

/// First named child of `node` whose kind matches.
pub(super) fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    (0..node.named_child_count())
        .filter_map(|i| node.named_child(i as u32))
        .find(|c| c.kind() == kind)
}

/// Named children of `node` as an owned vector.
pub(super) fn named_children<'a>(node: Node<'a>) -> Vec<Node<'a>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

/// Walk a C/C++ declarator chain to its terminal name. Function declarators
/// end at an `identifier`; typedef declarators at a `type_identifier`; C++
/// member names at a `field_identifier`. A qualified name (`Foo::bar`) resolves
/// to its trailing segment.
/// with a `declarator` field (`function_declarator`/`pointer_declarator`/
/// `array_declarator`) follow it; `attributed_declarator`/`parenthesized_declarator`
/// expose children, so descend via the first non-attribute named child. Depth-
/// capped (32) as defense-in-depth; falls back to the node's own text.
pub(super) fn declarator_name(decl: Node<'_>, source: &str) -> String {
    let mut cur = decl;
    for _ in 0..32 {
        match cur.kind() {
            "identifier" | "type_identifier" | "field_identifier" => {
                return cur
                    .utf8_text(source.as_bytes())
                    .map(|t| t.trim().to_string())
                    .unwrap_or_default();
            }
            "qualified_identifier" | "qualified_field_identifier" => {
                // `Foo::bar` -> `bar`: take the trailing identifier segment.
                return named_children(cur)
                    .into_iter()
                    .rev()
                    .find(|c| {
                        matches!(
                            c.kind(),
                            "identifier" | "field_identifier" | "type_identifier"
                        )
                    })
                    .and_then(|n| {
                        n.utf8_text(source.as_bytes())
                            .ok()
                            .map(|t| t.trim().to_string())
                    })
                    .unwrap_or_else(|| {
                        cur.utf8_text(source.as_bytes())
                            .ok()
                            .map(|t| t.trim().to_string())
                            .unwrap_or_default()
                    });
            }
            _ => match cur.child_by_field_name("declarator") {
                Some(inner) => cur = inner,
                None => match named_children(cur).into_iter().find(|c| {
                    !matches!(
                        c.kind(),
                        "attribute" | "attribute_declaration" | "attribute_specifier"
                    )
                }) {
                    Some(inner) => cur = inner,
                    None => {
                        return cur
                            .utf8_text(source.as_bytes())
                            .map(|t| t.trim().to_string())
                            .unwrap_or_default();
                    }
                },
            },
        }
    }
    cur.utf8_text(source.as_bytes())
        .map(|t| t.trim().to_string())
        .unwrap_or_default()
}
