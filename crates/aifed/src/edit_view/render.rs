//! Rendering of [`EditRow`] sequences into the `LINE:HASH|content` display format.
//!
//! The renderer only ever reads the content field of the variant it is formatting
//! — it never indexes any external array — so the coordinate-system bug that
//! motivated this module cannot recur here.

use super::model::EditRow;
use crate::escape::escape_for_display;
use crate::hash::hash_line;

/// Render a row sequence as a diff view.
///
/// With `context_lines > 0`, each changed row is shown together with up to
/// `context_lines` unchanged rows on either side; runs separated by hidden rows
/// are split by a blank line (the existing hunk-gap behaviour). With
/// `context_lines == 0`, only changed rows are emitted (the flat undo/redo shape).
pub fn render_rows(rows: &[EditRow], context_lines: usize) -> String {
    let changed: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.is_change())
        .map(|(i, _)| i)
        .collect();
    if changed.is_empty() {
        return "  (no changes)".to_string();
    }
    if context_lines == 0 {
        return changed
            .iter()
            .map(|&i| render_row(&rows[i]))
            .collect::<Vec<_>>()
            .join("\n");
    }
    let visible = visible_indices(rows, &changed, context_lines);
    let mut out = Vec::new();
    let mut prev: Option<usize> = None;
    for i in visible {
        if let Some(p) = prev
            && i > p + 1
        {
            out.push(String::new()); // blank separator between disjoint hunks
        }
        out.push(render_row(&rows[i]));
        prev = Some(i);
    }
    out.join("\n")
}

/// Indices of rows to show in a `context_lines > 0` view: every changed index plus
/// up to `context_lines` neighbours on each side, deduped and ascending. Isolated
/// from `render_rows` so the windowing policy is independently testable.
fn visible_indices(rows: &[EditRow], changed: &[usize], context_lines: usize) -> Vec<usize> {
    let mut show = vec![false; rows.len()];
    for &c in changed {
        let lo = c.saturating_sub(context_lines);
        let hi = (c + context_lines).min(rows.len().saturating_sub(1));
        show[lo..=hi].fill(true);
    }
    (0..rows.len()).filter(|&i| show[i]).collect()
}

/// Format a single row as `LINE:HASH|content`, prefixed by ` `/`+`/`-`.
///
/// The hash is computed from the row's own content, matching `aifed read`, so the
/// `LINE:HASH` prefix can be copied directly as a hashline anchor for the next edit.
fn render_row(r: &EditRow) -> String {
    match r {
        EditRow::Equal { new_line, content, .. } => {
            format!(
                " {}:{}|{}",
                new_line,
                hash_line(content),
                escape_for_display(content)
            )
        }
        EditRow::Insert { new_line, new_content } => {
            format!(
                "+{}:{}|{}",
                new_line,
                hash_line(new_content),
                escape_for_display(new_content)
            )
        }
        EditRow::Delete { old_line, old_content } => {
            format!(
                "-{}:{}|{}",
                old_line,
                hash_line(old_content),
                escape_for_display(old_content)
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eq(new: usize, c: &str) -> EditRow {
        EditRow::equal(new, c)
    }

    #[test]
    fn empty_or_all_unchanged_reports_no_changes() {
        assert_eq!(render_rows(&[], 3), "  (no changes)");
        assert_eq!(render_rows(&[eq(1, "a")], 3), "  (no changes)");
    }

    #[test]
    fn deletion_does_not_swallow_following_line() {
        // Bug #1 repro: deleting original line 4 must keep L5 visible at new line 4.
        let rows = vec![
            eq(1, "L1"),
            eq(2, "L2"),
            eq(3, "L3 content-before"),
            EditRow::delete(4, ""),
            eq(4, "L5 content-after"),
            eq(5, "L6"),
            eq(6, "L7"),
        ];
        let out = render_rows(&rows, 3);
        let l5_hash = hash_line("L5 content-after");
        assert!(
            out.contains(&format!(" 4:{l5_hash}|L5 content-after")),
            "L5 must remain visible at new line 4; got:\n{out}"
        );
        let blank_hash = hash_line("");
        assert!(out.contains(&format!("-4:{blank_hash}|")), "got:\n{out}");
    }

    #[test]
    fn replace_shows_both_old_and_new() {
        // Bug #2 repro: replace must render both the deletion and the insertion.
        let rows = vec![
            eq(1, "L1"),
            eq(2, "L2"),
            EditRow::delete(3, "L3"),
            EditRow::insert(3, "REPLACED"),
            eq(4, "L4"),
        ];
        let out = render_rows(&rows, 3);
        let l3_hash = hash_line("L3");
        let rep_hash = hash_line("REPLACED");
        assert!(out.contains(&format!("-3:{l3_hash}|L3")), "got:\n{out}");
        assert!(
            out.contains(&format!("+3:{rep_hash}|REPLACED")),
            "got:\n{out}"
        );
    }

    #[test]
    fn insert_uses_new_coordinates() {
        let rows = vec![eq(1, "L1"), eq(2, "L2"), EditRow::insert(3, "INSERTED"), eq(4, "L3")];
        let out = render_rows(&rows, 3);
        let ins_hash = hash_line("INSERTED");
        let l3_hash = hash_line("L3");
        assert!(
            out.contains(&format!("+3:{ins_hash}|INSERTED")),
            "got:\n{out}"
        );
        // Context after the insert uses the shifted (new) coordinate.
        assert!(out.contains(&format!(" 4:{l3_hash}|L3")), "got:\n{out}");
    }

    #[test]
    fn zero_context_emits_only_changes() {
        let rows = vec![eq(1, "L1"), EditRow::delete(2, "L2"), eq(3, "L3")];
        let out = render_rows(&rows, 0);
        assert!(!out.contains("L1"));
        assert!(!out.contains("L3"));
        let l2_hash = hash_line("L2");
        assert!(out.contains(&format!("-2:{l2_hash}|L2")), "got:\n{out}");
    }

    #[test]
    fn disjoint_hunks_separated_by_blank_line() {
        let rows = vec![
            EditRow::insert(1, "A"),
            eq(2, "ctx1"),
            eq(3, "ctx2"),
            eq(4, "ctx3"),
            EditRow::insert(5, "B"),
        ];
        let out = render_rows(&rows, 1);
        assert!(
            out.contains("\n\n"),
            "expected blank separator; got:\n{out}"
        );
    }

    #[test]
    fn render_row_hashline_matches_hash_line() {
        let rendered = render_row(&EditRow::insert(7, "fn main() {}"));
        assert_eq!(
            rendered,
            format!("+7:{}|fn main() {{}}", hash_line("fn main() {}"))
        );
    }

    #[test]
    fn visible_indices_expands_context_and_clamps() {
        // indices 0..6; changes at 1 and 5; context 1 -> {0,1,2} union {4,5}
        let rows: Vec<EditRow> = (0..6).map(|i| EditRow::equal(i + 1, "x")).collect();
        assert_eq!(visible_indices(&rows, &[1, 5], 1), vec![0, 1, 2, 4, 5]);
        // Context clamps at file boundaries.
        assert_eq!(visible_indices(&rows, &[0], 5), vec![0, 1, 2, 3, 4, 5]);
    }
}
