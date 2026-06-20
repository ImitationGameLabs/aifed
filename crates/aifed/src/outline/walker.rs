//! Generic tree-sitter outline walker driven by a per-language [`Spec`].
//!
//! Replaces the per-language `collect_items` / `map_item` / `make_item` /
//! `effective_start_line` quartet: a language provides a [`Spec`], the walker
//! parses, traverses, wires name/body, and attributes docs. [`extract`] is the
//! shared entry point (parser setup + traversal); per-language post-passes (e.g.
//! `rust::tile_top_level`) and the shared `prepend_file_header` run afterwards.
//! Returns RAW items (precise node ranges, pre-tiling).

use std::path::Path;

use tree_sitter::Node;

use super::model::OutlineItem;
use super::spec::{DocPolicy, Spec};
use crate::error::{Error, Result};

/// Parse `source` with `spec`'s grammar and walk it into raw outline items.
pub(super) fn extract<S: Spec>(
    spec: &S,
    source: &str,
    imports: bool,
    path: &Path,
) -> Result<Vec<OutlineItem>> {
    let mut parser = tree_sitter::Parser::new();
    let language = spec.language();
    parser
        .set_language(&language)
        .map_err(|e| Error::OutlineUnsupported {
            path: path.to_path_buf(),
            reason: format!("failed to load {} grammar: {e}", spec.grammar_name()),
        })?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| Error::OutlineUnsupported {
            path: path.to_path_buf(),
            reason: format!("{} parser returned no tree", spec.grammar_name()),
        })?;
    // Precompute source lines once; `effective_start_line` consults them per item.
    let lines: Vec<&str> = source.split('\n').collect();
    Ok(collect_items(
        tree.root_node(),
        source,
        &lines,
        imports,
        spec,
    ))
}

/// Walk `node`'s named children, mapping each via `spec` into outline items.
fn collect_items<S: Spec>(
    node: Node<'_>,
    source: &str,
    lines: &[&str],
    imports: bool,
    spec: &S,
) -> Vec<OutlineItem> {
    let policy = *spec.doc_policy();
    let mut items = Vec::new();
    for i in 0..node.named_child_count() {
        let Some(child) = node.named_child(i as u32) else {
            continue;
        };
        for expanded in spec.expand(child) {
            let Some(classified) = spec.classify(expanded, source) else {
                continue;
            };
            if classified.imports_gated && !imports {
                continue;
            }
            // Doc attribution walks back from the pre-expand `child` (the
            // container), not `expanded` — a container's doc/decorator siblings
            // sit at the container's level, not inside it.
            let start_line = effective_start_line(child, source, lines, policy);
            let end_line = expanded.end_position().row + 1;
            let (has_body, children) = match classified.body {
                Some(body) => (true, collect_items(body, source, lines, imports, spec)),
                None => (false, Vec::new()),
            };
            items.push(OutlineItem {
                kind: classified.kind,
                name: classified.name,
                start_line,
                end_line,
                has_body,
                level: None,
                detail: classified.detail,
                children,
            });
        }
    }
    items
}

/// Lowest 1-based start line of `node` plus any contiguous preceding outer doc
/// comments, attributes, or decorators, per `policy`.
///
/// Stops at the first non-attached sibling or a blank-line gap (when enabled),
/// or does not expand at all when `policy.expand_backward` is false (Python,
/// whose docstrings live in the body rather than above the item). Returns a
/// 1-based line directly so callers need no conversion.
fn effective_start_line(node: Node<'_>, source: &str, lines: &[&str], policy: DocPolicy) -> usize {
    if !policy.expand_backward {
        return node.start_position().row + 1;
    }
    let bytes = source.as_bytes();
    let mut min_row = node.start_position().row;
    let mut cur = node;
    while let Some(prev) = cur.prev_sibling() {
        let kind = prev.kind();
        let attach = policy.attribute_kinds.contains(&kind)
            || policy.decorator_kinds.contains(&kind)
            || (policy.attach_extras
                && prev.is_extra()
                && (policy.doc_prefixes.is_empty()
                    || matches!(prev.utf8_text(bytes),
                        Ok(t) if policy.doc_prefixes.iter().any(|p| t.starts_with(p)))));
        if !attach {
            break;
        }
        // Stop if a blank line separates `prev` from the attached region.
        // (Node end_row is unreliable here — comments include their trailing
        // newline — so detect blank lines from the source text.)
        let prev_start = prev.start_position().row;
        if policy.blank_line_breaks {
            let gap_has_blank =
                (prev_start + 1..min_row).any(|r| r < lines.len() && lines[r].trim().is_empty());
            if gap_has_blank {
                break;
            }
        }
        min_row = min_row.min(prev_start);
        cur = prev;
    }
    min_row + 1
}
