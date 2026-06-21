//! Text rendering for the `outline` command.
//!
//! An outline renders as a summary header (`path [language] N lines, M items`)
//! followed by an indented tree of `label name [start,end]` rows — every row
//! carries a bracketed `[start,end]` range (single-line items show `[N,N]`),
//! usable directly as the locator argument to `aifed read` and unambiguous as a
//! delimiter since `[` never appears in an identifier. For code, the top-level
//! ranges tile the whole file, and the leading preamble (the lines before the
//! first symbol) is summarized as a synthetic `file header` row, separated from
//! the symbol tree by a divider so it can't be read as a symbol. There is no
//! column alignment: the output targets agents, not humans. JSON lives in
//! `output::format_outline` via `serde`.

use super::model::{ItemKind, Outline, OutlineItem};

/// Standalone rule under the `file header` row. Box-drawing (not ASCII `---`) so
/// it can't collide with a setext-heading underline or YAML frontmatter; it
/// carries no locator and no `kind`, so it is outside the item-row grammar.
const FILE_HEADER_DIVIDER: &str = "────────────────────";

/// Render the outline as a summary header plus an indented tree of rows.
pub fn render_text(outline: &Outline) -> String {
    let n = count_items(&outline.items);
    let mut out = format!(
        "{} [{}] {} lines, {} {}",
        outline.path,
        outline.language,
        outline.total_lines,
        n,
        if n == 1 { "item" } else { "items" }
    );
    // A leading synthetic `file header` (the preamble before the first symbol)
    // is set off by a divider. Only the first item can be one; everything else
    // is the normal symbol tree.
    let mut items = outline.items.iter();
    if let Some(first) = items.next() {
        if first.kind == ItemKind::FileHeader {
            // Standard `[start,end]` locator, then a divider before the tree.
            out.push('\n');
            out.push_str(&display(first));
            out.push_str(&format!(" [{},{}]", first.start_line, first.end_line));
            out.push('\n');
            out.push_str(FILE_HEADER_DIVIDER);
        } else {
            render_item(first, 0, &mut out);
        }
    }
    for item in items {
        render_item(item, 0, &mut out);
    }
    out
}

fn render_item(item: &OutlineItem, depth: usize, out: &mut String) {
    out.push('\n');
    out.push_str(&"  ".repeat(depth));
    out.push_str(&display(item));
    // Every row is `[start,end]` (single-line items show `[N,N]`): a uniform,
    // copy-pasteable locator whose bracket can't be read as part of the name.
    out.push_str(&format!(" [{},{}]", item.start_line, item.end_line));
    for child in &item.children {
        render_item(child, depth + 1, out);
    }
}

/// Number of symbol nodes (root + nested), excluding the synthetic `file
/// header` — so the summary's `M items` counts navigable symbols, not the
/// preamble region.
fn count_items(items: &[OutlineItem]) -> usize {
    items
        .iter()
        .filter(|i| i.kind != ItemKind::FileHeader)
        .map(|i| 1 + count_items(&i.children))
        .sum()
}

/// Single-line label for a row, e.g. `fn main`, `impl Foo`, or `## Section`.
fn display(item: &OutlineItem) -> String {
    match item.kind {
        ItemKind::Heading => {
            let markers = "#".repeat(item.level.unwrap_or(1) as usize);
            format!("{} {}", markers, item.name)
        }
        // Synthetic region, not a symbol: render its multi-word label with no
        // `kind` prefix so it can't be mistaken for one.
        ItemKind::FileHeader => item.name.clone(),
        _ => format!("{} {}", label(item.kind), item.name),
    }
}

/// Short lowercase kind label (e.g. "fn"); `""` for kinds whose display is
/// fully derived from `name`/`level` (headings render as `#`-markers).
fn label(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Module => "mod",
        ItemKind::Function => "fn",
        ItemKind::Struct => "struct",
        ItemKind::Class => "class",
        ItemKind::Enum => "enum",
        ItemKind::Union => "union",
        ItemKind::Trait => "trait",
        ItemKind::Interface => "interface",
        ItemKind::Impl => "impl",
        ItemKind::TypeAlias => "type",
        ItemKind::Const => "const",
        ItemKind::Variable => "var",
        ItemKind::Static => "static",
        ItemKind::Macro => "macro",
        ItemKind::Extern => "extern",
        ItemKind::Heading => "",
        ItemKind::Imports => "use",
        ItemKind::FileHeader => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fn_item(name: &str, start: usize, end: usize) -> OutlineItem {
        OutlineItem {
            kind: ItemKind::Function,
            name: name.to_string(),
            start_line: start,
            end_line: end,
            has_body: false,
            level: None,
            detail: None,
            children: Vec::new(),
        }
    }

    fn outline(items: Vec<OutlineItem>) -> Outline {
        Outline { path: "t.rs".to_string(), language: "rust", total_lines: 30, items }
    }

    #[test]
    fn header_summarizes_lines_and_items() {
        let text = render_text(&outline(vec![fn_item("main", 10, 20)]));
        assert!(text.contains("t.rs [rust] 30 lines, 1 item"), "{text}");
    }

    #[test]
    fn multiline_item_shows_range() {
        let text = render_text(&outline(vec![fn_item("main", 10, 20)]));
        assert!(text.contains("\nfn main [10,20]"), "{text}");
    }

    #[test]
    fn single_line_item_shows_range() {
        // Single-line items use the same `[start,end]` form: `[5,5]`, never a
        // bare number, so the bracket delimits name from locator uniformly.
        let text = render_text(&outline(vec![fn_item("foo", 5, 5)]));
        assert!(text.contains("\nfn foo [5,5]"), "{text}");
        assert!(!text.contains("\nfn foo 5"), "{text}");
    }

    #[test]
    fn heading_renders_as_markers() {
        let outline = Outline {
            path: "t.md".to_string(),
            language: "markdown",
            total_lines: 1,
            items: vec![OutlineItem {
                kind: ItemKind::Heading,
                name: "Title".to_string(),
                start_line: 1,
                end_line: 1,
                has_body: false,
                level: Some(2),
                detail: None,
                children: Vec::new(),
            }],
        };
        assert!(render_text(&outline).contains("## Title [1,1]"));
    }

    #[test]
    fn indents_children() {
        let text = render_text(&outline(vec![OutlineItem {
            kind: ItemKind::Impl,
            name: "Foo".to_string(),
            start_line: 5,
            end_line: 9,
            has_body: true,
            level: None,
            detail: None,
            children: vec![fn_item("method", 6, 8)],
        }]));
        assert!(text.contains("\nimpl Foo [5,9]"), "{text}");
        assert!(text.contains("\n  fn method [6,8]"), "{text}");
    }

    fn file_header(start: usize, end: usize) -> OutlineItem {
        OutlineItem {
            kind: ItemKind::FileHeader,
            name: "file header".to_string(),
            start_line: start,
            end_line: end,
            has_body: false,
            level: None,
            detail: None,
            children: Vec::new(),
        }
    }

    #[test]
    fn file_header_row_renders_with_divider() {
        let text = render_text(&outline(vec![file_header(1, 23), fn_item("main", 24, 40)]));
        // Region form `[1,23]` even though it's the first row; multi-word label;
        // a box-drawing divider sits between it and the first symbol.
        assert!(text.contains("\nfile header [1,23]"), "{text}");
        assert!(text.contains('\u{2500}'), "{text}");
        let header_at = text.find("file header [1,23]").unwrap();
        let fn_at = text.find("fn main").unwrap();
        assert!(
            header_at < fn_at,
            "header must precede the symbol tree:\n{text}"
        );
    }

    #[test]
    fn no_divider_without_file_header() {
        let text = render_text(&outline(vec![fn_item("main", 1, 5)]));
        assert!(!text.contains('\u{2500}'), "{text}");
        assert!(!text.contains("file header"), "{text}");
    }

    #[test]
    fn file_header_excluded_from_item_count() {
        let text = render_text(&outline(vec![
            file_header(1, 5),
            fn_item("main", 6, 9),
            fn_item("bar", 10, 12),
        ]));
        // header + 2 fns, but the header is synthetic → "2 items".
        assert!(text.contains(" 2 items"), "{text}");
    }
}
