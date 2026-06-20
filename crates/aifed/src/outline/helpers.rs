//! Shared tree-sitter node helpers used by every language's [`super::spec::Spec`].

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
