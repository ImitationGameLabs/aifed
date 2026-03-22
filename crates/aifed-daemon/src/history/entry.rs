//! History entry and line diff types

use time::OffsetDateTime;

/// Represents a single line change in a file edit operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineDiff {
    /// Line number in the original file (1-based)
    pub line_num: usize,
    /// Hash of the original line content (for verification)
    pub old_hash: Option<String>,
    /// Original line content (None for insertions)
    pub old_content: Option<String>,
    /// New line content (None for deletions)
    pub new_content: Option<String>,
}

impl LineDiff {
    /// Create a diff for a line replacement.
    pub fn for_replacement(
        line_num: usize,
        old_hash: &str,
        old_content: &str,
        new_content: &str,
    ) -> Self {
        Self {
            line_num,
            old_hash: Some(old_hash.to_string()),
            old_content: Some(old_content.to_string()),
            new_content: Some(new_content.to_string()),
        }
    }

    /// Create a diff for a line insertion.
    pub fn for_insertion(line_num: usize, new_content: &str) -> Self {
        Self {
            line_num,
            old_hash: None,
            old_content: None,
            new_content: Some(new_content.to_string()),
        }
    }

    /// Create a diff for a line deletion.
    pub fn for_deletion(line_num: usize, old_hash: &str, old_content: &str) -> Self {
        Self {
            line_num,
            old_hash: Some(old_hash.to_string()),
            old_content: Some(old_content.to_string()),
            new_content: None,
        }
    }
}

/// Represents a single edit operation in the history.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// Unique identifier for this entry
    pub id: u64,
    /// Timestamp of the edit
    pub timestamp: OffsetDateTime,
    /// Human-readable summary (e.g., "+3 lines", "+1, -2 lines")
    pub summary: String,
    /// The line diffs for this edit
    pub diffs: Vec<LineDiff>,
}

impl HistoryEntry {
    /// Create a new history entry with auto-generated summary.
    pub fn new(id: u64, diffs: Vec<LineDiff>) -> Self {
        let summary = Self::generate_summary(&diffs);
        Self { id, timestamp: OffsetDateTime::now_utc(), summary, diffs }
    }

    /// Generate a summary string from the diffs.
    ///
    /// Replacements are counted as 1 deletion + 1 insertion (git-style).
    fn generate_summary(diffs: &[LineDiff]) -> String {
        let mut insertions = 0;
        let mut deletions = 0;

        for diff in diffs {
            match (&diff.old_content, &diff.new_content) {
                (None, Some(_)) => insertions += 1, // Pure insertion
                (Some(_), None) => deletions += 1,  // Pure deletion
                (Some(_), Some(_)) => {
                    // Replacement = 1 del + 1 ins
                    deletions += 1;
                    insertions += 1;
                }
                (None, None) => {} // Should not happen
            }
        }

        let mut parts = Vec::new();
        if insertions > 0 {
            parts.push(format!(
                "{} insertion{}(+)",
                insertions,
                if insertions > 1 { "s" } else { "" }
            ));
        }
        if deletions > 0 {
            parts.push(format!(
                "{} deletion{}(-)",
                deletions,
                if deletions > 1 { "s" } else { "" }
            ));
        }

        if parts.is_empty() { "no changes".to_string() } else { parts.join(", ") }
    }
}
