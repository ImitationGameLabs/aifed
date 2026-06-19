//! Pure-data row model for edit-result / rename / undo-redo / history diff DISPLAY.
//!
//! Each [`EditRow`] carries its own content and its own correct line coordinate(s),
//! so a renderer never indexes a foreign array. This makes the historical
//! coordinate-mixing bug — where `delete` changes used original-file line numbers
//! but the renderer indexed the post-edit file — unrepresentable by construction.
//!
//! Model/render separation mirrors `crate::outline`.

/// One displayable row of an edit view.
///
/// Variants encode their coordinate contract: an [`EditRow::Delete`] carries only
/// an original-file coordinate (the line has no post-edit position), and an
/// [`EditRow::Insert`] carries only a new-file coordinate. A renderer therefore
/// cannot accidentally use a removed line's number to index the post-edit file —
/// the class of bug this module exists to prevent.
///
/// Note: only the field *shape* is type-enforced (a `Delete` has no `new_line`
/// to misuse). The constructors' 1-based `debug_assert` is a dev-only sanity
/// check — enum variant fields are public, so direct construction bypasses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditRow {
    /// Unchanged line carried from the original file into the new file.
    /// `new_line` is the coordinate shown to the user (matches `aifed read`).
    Equal { new_line: usize, content: String },
    /// Line present only in the original file (removed). No new-file coordinate.
    Delete { old_line: usize, old_content: String },
    /// Line present only in the new file (added). No original coordinate.
    Insert { new_line: usize, new_content: String },
}

impl EditRow {
    /// Context line: present unchanged in both files. `new_line` is 1-based.
    pub fn equal(new_line: usize, content: impl Into<String>) -> Self {
        debug_assert!(new_line >= 1, "lines are 1-based");
        EditRow::Equal { new_line, content: content.into() }
    }

    /// Removed line; `old_line` is its 1-based original-file position.
    pub fn delete(old_line: usize, old_content: impl Into<String>) -> Self {
        debug_assert!(old_line >= 1, "lines are 1-based");
        EditRow::Delete { old_line, old_content: old_content.into() }
    }

    /// Added line; `new_line` is its 1-based new-file position.
    pub fn insert(new_line: usize, new_content: impl Into<String>) -> Self {
        debug_assert!(new_line >= 1, "lines are 1-based");
        EditRow::Insert { new_line, new_content: new_content.into() }
    }

    /// Whether this row represents a change (not unchanged context).
    pub fn is_change(&self) -> bool {
        !matches!(self, EditRow::Equal { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_build_expected_variants() {
        assert_eq!(
            EditRow::equal(1, "x"),
            EditRow::Equal { new_line: 1, content: "x".to_string() }
        );
        assert_eq!(
            EditRow::delete(4, ""),
            EditRow::Delete { old_line: 4, old_content: String::new() }
        );
        assert_eq!(
            EditRow::insert(3, "REPLACED"),
            EditRow::Insert { new_line: 3, new_content: "REPLACED".to_string() }
        );
    }

    #[test]
    fn is_change_distinguishes_context() {
        assert!(!EditRow::equal(1, "x").is_change());
        assert!(EditRow::delete(1, "x").is_change());
        assert!(EditRow::insert(1, "x").is_change());
    }
}
