//! Markdown outline extraction via tree-sitter (block grammar only).
//!
//! Headings form a tree by level, and each heading's range spans its whole
//! *section* — the heading line through the line before the next heading at the
//! same or shallower level (or end of file). This mirrors how a Rust item's
//! range covers its body, so `aifed read <FILE> [start,end]` returns the full
//! section, not just the heading line.
//!
//! The block grammar's `document` root only directly contains `section`/metadata
//! nodes, so the whole tree is walked pre-order to surface headings wherever
//! nested. Code blocks are intentionally not emitted: a heading's section range
//! already covers them, and listing every fence would clutter the outline.

use std::path::Path;

use tree_sitter::Node;

use super::model::{ItemKind, OutlineItem};
use crate::error::{Error, Result};

/// Extract a Markdown outline: a heading tree with section-spanning ranges.
pub fn extract(source: &str, path: &Path) -> Result<Vec<OutlineItem>> {
    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter_md::LANGUAGE.into();
    parser
        .set_language(&language)
        .map_err(|e| Error::OutlineUnsupported {
            path: path.to_path_buf(),
            reason: format!("failed to load markdown grammar: {e}"),
        })?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| Error::OutlineUnsupported {
            path: path.to_path_buf(),
            reason: "markdown parser returned no tree".to_string(),
        })?;

    let mut headings = Vec::new();
    walk(tree.root_node(), source, &mut headings);
    let lines: Vec<&str> = source.split('\n').collect();
    Ok(build_tree(headings, &lines))
}

/// A heading collected during the walk, in document order. Its section end is
/// computed later (against the next competing heading or end of file).
struct HeadingEntry {
    level: u8,
    text: String,
    start: usize,
}

/// Pre-order walk over every node, collecting ATX and setext headings.
fn walk(node: Node<'_>, source: &str, out: &mut Vec<HeadingEntry>) {
    let start = node.start_position().row + 1;
    match node.kind() {
        "atx_heading" => out.push(HeadingEntry {
            level: marker_level(node, "atx_h", "_marker").unwrap_or(1),
            text: heading_text(node, source),
            start,
        }),
        "setext_heading" => out.push(HeadingEntry {
            level: marker_level(node, "setext_h", "_underline").unwrap_or(2),
            text: heading_text(node, source),
            start,
        }),
        _ => {}
    }
    // Headings live under `section` nodes — recurse everywhere to find them.
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            walk(child, source, out);
        }
    }
}

/// Build the heading tree, assigning each heading a section-spanning range.
///
/// A level-L heading's section runs from its line to the line before the next
/// heading at level <= L (or end of file). Trailing blank lines are trimmed.
fn build_tree(headings: Vec<HeadingEntry>, lines: &[&str]) -> Vec<OutlineItem> {
    let total_lines = lines.len();
    let mut nodes: Vec<OutlineItem> = Vec::new();
    let mut parents: Vec<Option<usize>> = Vec::new();
    // Open headings as (level, arena index). A heading closes (and gets its
    // section end pinned) when a later heading at the same or shallower level arrives.
    let mut stack: Vec<(u8, usize)> = Vec::new();

    for heading in headings {
        while let Some(&(top_level, top_idx)) = stack.last() {
            if top_level < heading.level {
                break;
            }
            // `top` is same/deeper level — its section ends just before this heading.
            nodes[top_idx].end_line = heading.start.saturating_sub(1);
            stack.pop();
        }
        let parent = stack.last().map(|&(_, idx)| idx);
        let idx = nodes.len();
        nodes.push(OutlineItem {
            kind: ItemKind::Heading,
            name: heading.text,
            start_line: heading.start,
            end_line: total_lines, // default: section runs to end of file
            has_body: false,
            level: Some(heading.level),
            detail: None,
            children: Vec::new(),
        });
        parents.push(parent);
        stack.push((heading.level, idx));
    }
    // Headings left on the stack keep `total_lines`; trim trailing blanks from
    // every section end (both closed and EOF) for tight, content-accurate ranges.
    for node in nodes.iter_mut() {
        while node.end_line > node.start_line
            && node.end_line <= lines.len()
            && lines[node.end_line - 1].trim().is_empty()
        {
            node.end_line -= 1;
        }
    }

    let mut children_of: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (child_idx, parent) in parents.iter().enumerate() {
        if let Some(p) = parent {
            children_of[*p].push(child_idx);
        }
    }

    fn build(idx: usize, nodes: &[OutlineItem], children_of: &[Vec<usize>]) -> OutlineItem {
        let mut item = nodes[idx].clone();
        for &child in &children_of[idx] {
            item.children.push(build(child, nodes, children_of));
        }
        item
    }

    parents
        .iter()
        .enumerate()
        .filter(|(_, p)| p.is_none())
        .map(|(idx, _)| build(idx, &nodes, &children_of))
        .collect()
}

/// Parse the heading level from its marker/underline child kind.
/// `prefix`/`suffix` select ATX (`atx_h`..`_marker`) vs setext (`setext_h`..`_underline`).
fn marker_level(node: Node<'_>, prefix: &str, suffix: &str) -> Option<u8> {
    for i in 0..node.child_count() {
        if let Some(c) = node.child(i as u32)
            && let Some(rest) = c.kind().strip_prefix(prefix)
            && let Some(num) = rest.strip_suffix(suffix)
            && let Ok(level) = num.parse::<u8>()
        {
            return Some(level);
        }
    }
    None
}

/// Visible text of a heading. For ATX the `heading_content` field is an
/// `inline` node; for setext it is a `paragraph` wrapping `inline` children.
/// Recursing to collect `inline` text handles both, skipping `block_continuation`.
fn heading_text(node: Node<'_>, source: &str) -> String {
    let Some(content) = node.child_by_field_name("heading_content") else {
        return String::new();
    };
    let mut text = String::new();
    collect_inline(content, source, &mut text);
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collect_inline(node: Node<'_>, source: &str, out: &mut String) {
    if node.kind() == "inline" {
        if let Ok(t) = node.utf8_text(source.as_bytes()) {
            out.push_str(t);
        }
        return;
    }
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i as u32) {
            collect_inline(child, source, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn extract(src: &str) -> Vec<OutlineItem> {
        super::extract(src, Path::new("test.md")).unwrap()
    }

    #[test]
    fn heading_ranges_span_their_sections() {
        // lines: 1 #A | 2 para | 3 ##B | 4 b-text | 5 ##C | 6 c-text | 7 (trailing)
        let items = extract("# A\npara\n## B\nb-text\n## C\nc-text\n");
        let a = &items[0];
        assert_eq!((a.name.as_str(), a.start_line, a.end_line), ("A", 1, 6)); // whole doc
        let b = &a.children[0];
        assert_eq!((b.name.as_str(), b.start_line, b.end_line), ("B", 3, 4)); // until ## C
        let c = &a.children[1];
        assert_eq!((c.name.as_str(), c.start_line, c.end_line), ("C", 5, 6)); // to EOF
    }

    #[test]
    fn atx_levels_and_text() {
        let items = extract("# Title\n## Sub\n### Deep\n");
        let title = &items[0];
        assert_eq!(
            (title.kind, title.name.as_str(), title.level),
            (ItemKind::Heading, "Title", Some(1))
        );
        let sub = &title.children[0];
        assert_eq!((sub.name.as_str(), sub.level), ("Sub", Some(2)));
        assert_eq!(
            (sub.children[0].name.as_str(), sub.children[0].level),
            ("Deep", Some(3))
        );
    }

    #[test]
    fn setext_heading_range_spans_section() {
        // lines: 1 Title | 2 ===== | 3 body | 4 (trailing)
        let items = extract("Title\n=====\nbody\n");
        let h = &items[0];
        assert_eq!((h.name.as_str(), h.start_line, h.end_line), ("Title", 1, 3));
    }

    #[test]
    fn multiline_setext_heading_range() {
        // "Foo\nbar\n---\nbody" -> text 1-2, underline 3, body 4.
        let items = extract("Foo\nbar\n---\nbody\n");
        assert_eq!((items[0].start_line, items[0].end_line), (1, 4));
    }

    #[test]
    fn heading_stack_pops_on_level_decrease() {
        let items = extract("# A\n## B\n# C\n");
        assert_eq!(items.len(), 2, "{items:?}");
        assert_eq!(items[0].name, "A");
        assert_eq!(items[0].children[0].name, "B");
        assert_eq!(items[1].name, "C");
    }

    #[test]
    fn content_less_heading_is_single_line() {
        // Two adjacent headings with nothing between: A's section is just its line.
        let items = extract("## A\n## B\n");
        assert_eq!((items[0].start_line, items[0].end_line), (1, 1));
    }

    #[test]
    fn trailing_blank_lines_trimmed() {
        // A's section ends before # B at line 5; lines 3-4 are blank and must be trimmed.
        let items = extract("# A\nx\n\n\n# B\n");
        assert_eq!((items[0].start_line, items[0].end_line), (1, 2));
    }

    #[test]
    fn empty_markdown() {
        assert!(extract("").is_empty());
    }

    #[test]
    fn ranges_are_locator_compatible() {
        let items = extract("# A\n## B\nbody\n");
        fn check(items: &[OutlineItem]) {
            for item in items {
                let loc = format!("[{},{}]", item.start_line, item.end_line);
                crate::locator::Locator::parse(&loc).unwrap();
                check(&item.children);
            }
        }
        check(&items);
    }
}
