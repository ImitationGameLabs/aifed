//! Edit-view display model shared by the edit result, rename, undo/redo, and
//! history commands.
//!
//! Every display path builds an [`EditRow`] sequence — each row carries its own
//! content and correct line coordinate(s) — then renders it via
//! [`render::render_rows`]. Because renderers never index a foreign array, the
//! coordinate-system confusion that previously broke deletions/replacements in the
//! edit-result view is structurally impossible.
//!
//! Model/render separation mirrors `crate::outline`.

mod model;
mod render;

pub use model::EditRow;
pub(crate) use render::render_rows;

use aifed_common::LineDiffDto;

/// Recover the post-edit file contents from a row sequence: the content of every
/// `Equal` and `Insert` row, in order. `Delete` rows contribute nothing.
pub fn new_lines_from_rows(rows: &[EditRow]) -> Vec<String> {
    rows.iter()
        .filter_map(|r| match r {
            EditRow::Equal { content, .. } => Some(content.clone()),
            EditRow::Insert { new_content, .. } => Some(new_content.clone()),
            EditRow::Delete { .. } => None,
        })
        .collect()
}

/// Build a changed-rows-only sequence from stored [`LineDiffDto`] (undo/redo and
/// history). Each diff maps to one or two rows via the same `(old, new)` arm logic
/// the old per-call-site matchers used.
pub fn changed_rows_from_diffs(diffs: &[LineDiffDto]) -> Vec<EditRow> {
    let mut rows = Vec::new();
    for d in diffs {
        match (&d.old_content, &d.new_content) {
            (None, Some(new)) => rows.push(EditRow::insert(d.line_num, new)),
            (Some(old), None) => rows.push(EditRow::delete(d.line_num, old)),
            (Some(old), Some(new)) => {
                rows.push(EditRow::delete(d.line_num, old));
                rows.push(EditRow::insert(d.line_num, new));
            }
            (None, None) => {}
        }
    }
    rows
}

/// Build a full row sequence (changed and unchanged) by positionally aligning two
/// line arrays. Used by rename, which has both the original and the post-edit
/// content. Walking both arrays at the same index makes every row's coordinate
/// correct by construction — no foreign-array indexing.
pub fn rows_from_old_and_new(original: &[&str], new_lines: &[&str]) -> Vec<EditRow> {
    let max_lines = original.len().max(new_lines.len());
    let mut rows = Vec::new();
    for i in 0..max_lines {
        let line = i + 1;
        match (original.get(i), new_lines.get(i)) {
            (Some(&old), Some(&new)) => {
                if old == new {
                    rows.push(EditRow::equal(line, new));
                } else {
                    rows.push(EditRow::delete(line, old));
                    rows.push(EditRow::insert(line, new));
                }
            }
            (Some(&old), None) => rows.push(EditRow::delete(line, old)),
            (None, Some(&new)) => rows.push(EditRow::insert(line, new)),
            (None, None) => {}
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_diff(line_num: usize, old: Option<&str>, new: Option<&str>) -> LineDiffDto {
        LineDiffDto {
            line_num,
            old_hash: None,
            old_content: old.map(String::from),
            new_content: new.map(String::from),
        }
    }

    #[test]
    fn new_lines_skips_deletes() {
        let rows = vec![EditRow::equal(1, "a"), EditRow::delete(2, "b"), EditRow::insert(2, "c")];
        assert_eq!(
            new_lines_from_rows(&rows),
            vec!["a".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn changed_rows_from_diffs_maps_each_arm() {
        let diffs = vec![
            make_diff(1, None, Some("ins")),
            make_diff(2, Some("del"), None),
            make_diff(3, Some("old"), Some("new")),
        ];
        let rows = changed_rows_from_diffs(&diffs);
        assert_eq!(rows.len(), 4);
        assert!(matches!(rows[0], EditRow::Insert { new_line: 1, .. }));
        assert!(matches!(rows[1], EditRow::Delete { old_line: 2, .. }));
        assert!(matches!(rows[2], EditRow::Delete { old_line: 3, .. }));
        assert!(matches!(rows[3], EditRow::Insert { new_line: 3, .. }));
    }

    #[test]
    fn rows_from_old_and_new_aligns_positionally() {
        let original = ["a", "b", "c"];
        let new = ["a", "B", "c"];
        let rows = rows_from_old_and_new(&original, &new);
        assert!(matches!(rows[0], EditRow::Equal { new_line: 1, .. }));
        assert!(matches!(rows[1], EditRow::Delete { old_line: 2, .. }));
        assert!(matches!(rows[2], EditRow::Insert { new_line: 2, .. }));
        assert!(matches!(rows[3], EditRow::Equal { new_line: 3, .. }));
    }

    #[test]
    fn rows_from_old_and_new_handles_length_change() {
        let original = ["a", "b"];
        let new = ["a", "b", "c"];
        let rows = rows_from_old_and_new(&original, &new);
        assert_eq!(rows.len(), 3);
        assert!(matches!(rows[2], EditRow::Insert { new_line: 3, .. }));
    }
}
