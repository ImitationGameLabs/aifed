//! History manager for tracking file edits

use super::{HistoryEntry, LineDiff};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

/// Errors that can occur during history operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HistoryError {
    /// The file's hash has changed since last access (external modification)
    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    /// No history available for the file
    #[error("No history available")]
    NoHistory,

    /// No redo available for the file
    #[error("No redo available")]
    NoRedo,
}

/// Result of an undo/redo operation.
#[derive(Debug, Clone)]
pub struct UndoRedoResult {
    /// The diffs to apply to the file
    pub diffs: Vec<LineDiff>,
    /// The current hash of the file (for verification)
    pub current_hash: String,
}

/// History tracking for a single file.
#[derive(Debug, Clone)]
pub struct FileHistory {
    /// Last known hash of the file (from last aifed access)
    pub last_known_hash: String,
    /// Stack of undo entries (most recent at the end)
    pub undo_stack: Vec<HistoryEntry>,
    /// Stack of redo entries (most recent at the end)
    pub redo_stack: Vec<HistoryEntry>,
}

impl FileHistory {
    /// Create a new file history.
    pub fn new() -> Self {
        Self { last_known_hash: String::new(), undo_stack: Vec::new(), redo_stack: Vec::new() }
    }

    /// Update the last known hash.
    pub fn update_hash(&mut self, hash: &str) {
        self.last_known_hash = hash.to_string();
    }
}

/// Manages history for all files in the workspace.
pub struct HistoryManager {
    /// File histories indexed by path
    files: RwLock<HashMap<PathBuf, FileHistory>>,
    /// Maximum number of undo entries per file
    max_entries: usize,
    /// Next entry ID
    next_id: AtomicU64,
}

impl HistoryManager {
    /// Create a new history manager with default settings.
    pub fn new() -> Self {
        Self { files: RwLock::new(HashMap::new()), max_entries: 50, next_id: AtomicU64::new(1) }
    }

    /// Record a file access (read operation).
    /// This updates the last known hash for concurrent modification detection.
    pub fn record_access(&self, path: &Path, hash: &str) -> Result<(), HistoryError> {
        let mut files = self.files.write().unwrap();

        let history = files
            .entry(path.to_path_buf())
            .or_insert_with(FileHistory::new);

        history.update_hash(hash);
        Ok(())
    }

    /// Record an edit operation.
    /// Verifies the hash matches the last known hash before recording.
    pub fn record_edit(
        &self,
        path: &Path,
        expected_hash: &str,
        new_hash: &str,
        diffs: Vec<LineDiff>,
    ) -> Result<(), HistoryError> {
        let mut files = self.files.write().unwrap();

        let history = files
            .entry(path.to_path_buf())
            .or_insert_with(FileHistory::new);

        // Verify hash matches (skip if first access - empty last_known_hash)
        if !history.last_known_hash.is_empty() && history.last_known_hash != expected_hash {
            return Err(HistoryError::HashMismatch {
                expected: history.last_known_hash.clone(),
                actual: expected_hash.to_string(),
            });
        }

        // Create history entry
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let entry = HistoryEntry::new(id, diffs);

        // Add to undo stack
        history.undo_stack.push(entry);

        // Clear redo stack (new edit invalidates redo history)
        history.redo_stack.clear();

        // Evict old entries if over limit
        if history.undo_stack.len() > self.max_entries {
            history.undo_stack.remove(0);
        }

        // Update hash
        history.update_hash(new_hash);

        Ok(())
    }

    /// Compute inverse diffs for undo (swap old and new)
    fn compute_inverse_diffs(diffs: &[LineDiff]) -> Vec<LineDiff> {
        diffs
            .iter()
            .map(|diff| match (&diff.old_content, &diff.new_content) {
                (None, Some(new)) => {
                    // Insertion -> Deletion
                    LineDiff::for_deletion(
                        diff.line_num,
                        diff.old_hash.as_deref().unwrap_or(""),
                        new,
                    )
                }
                (Some(old), None) => {
                    // Deletion -> Insertion
                    LineDiff::for_insertion(diff.line_num, old)
                }
                (Some(_old), Some(_new)) => {
                    // Replacement -> Inverse replacement
                    LineDiff::for_replacement(diff.line_num, "", _new, _old)
                }
                (None, None) => LineDiff::for_replacement(diff.line_num, "", "", ""),
            })
            .collect()
    }

    /// Undo the last edit for a file.
    /// Returns the diffs to apply to undo the change and the current file hash.
    /// If dry_run is true, returns diffs without modifying the stacks.
    pub fn undo(&self, path: &Path, dry_run: bool) -> Result<UndoRedoResult, HistoryError> {
        let mut files = self.files.write().unwrap();

        let history = files.get_mut(path).ok_or(HistoryError::NoHistory)?;

        if history.undo_stack.is_empty() {
            return Err(HistoryError::NoHistory);
        }

        let current_hash = history.last_known_hash.clone();

        if dry_run {
            // Just peek and compute inverse diffs
            let entry = history.undo_stack.last().unwrap();
            return Ok(UndoRedoResult {
                diffs: Self::compute_inverse_diffs(&entry.diffs),
                current_hash,
            });
        }

        // Pop from undo stack
        let entry = history.undo_stack.pop().unwrap();

        // Compute inverse diffs
        let inverse_diffs = Self::compute_inverse_diffs(&entry.diffs);

        // Push to redo stack
        history.redo_stack.push(entry);

        Ok(UndoRedoResult { diffs: inverse_diffs, current_hash })
    }

    /// Redo the last undone edit for a file.
    /// Returns the diffs to apply to redo the change and the current file hash.
    /// If dry_run is true, returns diffs without modifying the stacks.
    pub fn redo(&self, path: &Path, dry_run: bool) -> Result<UndoRedoResult, HistoryError> {
        let mut files = self.files.write().unwrap();

        let history = files.get_mut(path).ok_or(HistoryError::NoRedo)?;

        if history.redo_stack.is_empty() {
            return Err(HistoryError::NoRedo);
        }

        let current_hash = history.last_known_hash.clone();

        if dry_run {
            // Just peek and return original diffs
            let entry = history.redo_stack.last().unwrap();
            return Ok(UndoRedoResult { diffs: entry.diffs.clone(), current_hash });
        }

        // Pop from redo stack
        let entry = history.redo_stack.pop().unwrap();

        // Clone the diffs (original diffs, not inverse)
        let diffs = entry.diffs.clone();

        // Push back to undo stack
        history.undo_stack.push(entry);

        Ok(UndoRedoResult { diffs, current_hash })
    }

    /// Get history entries for a file.
    /// Returns entries in reverse chronological order (most recent first).
    pub fn get_history(&self, path: &Path, count: Option<usize>) -> Vec<HistoryEntry> {
        let files = self.files.read().unwrap();

        let Some(history) = files.get(path) else {
            return Vec::new();
        };

        let entries: Vec<HistoryEntry> = history
            .undo_stack
            .iter()
            .rev()
            .take(count.unwrap_or(usize::MAX))
            .cloned()
            .collect();

        entries
    }
}

impl Default for HistoryManager {
    fn default() -> Self {
        Self::new()
    }
}
