//! Unit tests for history module

use super::*;

// ============================================================================
// LineDiff Tests
// ============================================================================

#[test]
fn test_line_diff_for_replacement() {
    let diff = LineDiff::for_replacement(10, "AB", "old content", "new content");

    assert_eq!(diff.line_num, 10);
    assert_eq!(diff.old_hash, Some("AB".to_string()));
    assert_eq!(diff.old_content, Some("old content".to_string()));
    assert_eq!(diff.new_content, Some("new content".to_string()));
}

#[test]
fn test_line_diff_for_insertion() {
    let diff = LineDiff::for_insertion(10, "new line content");

    assert_eq!(diff.line_num, 10);
    assert_eq!(diff.old_hash, None);
    assert_eq!(diff.old_content, None); // No old content for insertion
    assert_eq!(diff.new_content, Some("new line content".to_string()));
}

#[test]
fn test_line_diff_for_deletion() {
    let diff = LineDiff::for_deletion(10, "AB", "deleted content");

    assert_eq!(diff.line_num, 10);
    assert_eq!(diff.old_hash, Some("AB".to_string()));
    assert_eq!(diff.old_content, Some("deleted content".to_string()));
    assert_eq!(diff.new_content, None); // No new content for deletion
}

// ============================================================================
// HistoryEntry Tests
// ============================================================================

#[test]
fn test_history_entry_creation() {
    let entry = HistoryEntry::new(
        1,
        vec![LineDiff::for_insertion(10, "line 1"), LineDiff::for_insertion(11, "line 2")],
    );

    assert_eq!(entry.id, 1);
    assert_eq!(entry.diffs.len(), 2);
}

#[test]
fn test_history_entry_summary_insertions() {
    let entry = HistoryEntry::new(
        1,
        vec![
            LineDiff::for_insertion(10, "line 1"),
            LineDiff::for_insertion(11, "line 2"),
            LineDiff::for_insertion(12, "line 3"),
        ],
    );

    assert_eq!(entry.summary, "3 insertions(+)");
}

#[test]
fn test_history_entry_summary_deletions() {
    let entry = HistoryEntry::new(
        1,
        vec![
            LineDiff::for_deletion(10, "AB", "line 1"),
            LineDiff::for_deletion(11, "CD", "line 2"),
        ],
    );

    assert_eq!(entry.summary, "2 deletions(-)");
}

#[test]
fn test_history_entry_summary_replacements() {
    // Replacements count as 1 deletion + 1 insertion each
    let entry = HistoryEntry::new(
        1,
        vec![
            LineDiff::for_replacement(10, "AB", "old", "new"),
            LineDiff::for_replacement(11, "CD", "old2", "new2"),
            LineDiff::for_replacement(12, "EF", "old3", "new3"),
        ],
    );

    assert_eq!(entry.summary, "3 insertions(+), 3 deletions(-)");
}

#[test]
fn test_history_entry_summary_mixed() {
    // 1 insertion + 1 deletion + 1 replacement (which is 1 del + 1 ins)
    // Total: 2 insertions, 2 deletions
    let entry = HistoryEntry::new(
        1,
        vec![
            LineDiff::for_insertion(10, "new"),
            LineDiff::for_deletion(11, "AB", "old"),
            LineDiff::for_replacement(12, "CD", "old2", "new2"),
        ],
    );

    assert_eq!(entry.summary, "2 insertions(+), 2 deletions(-)");
}

// ============================================================================
// FileHistory Tests
// ============================================================================

#[test]
fn test_file_history_creation() {
    let history = FileHistory::new();

    assert_eq!(history.last_known_hash, "");
    assert!(history.undo_stack.is_empty());
    assert!(history.redo_stack.is_empty());
}

#[test]
fn test_file_history_update_hash() {
    let mut history = FileHistory::new();
    history.update_hash("ABC123");

    assert_eq!(history.last_known_hash, "ABC123");
}

// ============================================================================
// HistoryManager Tests
// ============================================================================

#[test]
fn test_manager_record_access() {
    let manager = HistoryManager::new();
    let path = std::path::PathBuf::from("/test/file.rs");

    manager.record_access(&path, "ABC123").unwrap();

    // Verify by doing an edit with matching hash
    let diffs = vec![LineDiff::for_insertion(10, "new line")];
    // This should succeed since hash matches
    manager.record_edit(&path, "ABC123", "DEF456", diffs).unwrap();
}

#[test]
fn test_manager_record_edit_new_file() {
    let manager = HistoryManager::new();
    let path = std::path::PathBuf::from("/test/file.rs");

    // First access
    manager.record_access(&path, "ABC123").unwrap();

    // Record edit (should succeed because hash matches)
    let diffs = vec![LineDiff::for_insertion(10, "new line")];
    manager.record_edit(&path, "ABC123", "DEF456", diffs).unwrap();

    // Verify history was recorded by undoing
    let result = manager.undo(&path, false).unwrap();
    assert_eq!(result.diffs.len(), 1);
}

#[test]
fn test_manager_record_edit_hash_mismatch() {
    let manager = HistoryManager::new();
    let path = std::path::PathBuf::from("/test/file.rs");

    // First access with hash ABC123
    manager.record_access(&path, "ABC123").unwrap();

    // Try to edit with wrong hash (file was modified externally)
    let diffs = vec![LineDiff::for_insertion(10, "new line")];
    let result = manager.record_edit(&path, "WRONG_HASH", "DEF456", diffs);

    assert!(result.is_err());
    match result.unwrap_err() {
        HistoryError::HashMismatch { expected, actual } => {
            assert_eq!(expected, "ABC123");
            assert_eq!(actual, "WRONG_HASH");
        }
        _ => panic!("Expected HashMismatch error"),
    }
}

#[test]
fn test_manager_undo() {
    let manager = HistoryManager::new();
    let path = std::path::PathBuf::from("/test/file.rs");

    // Setup: record access and edit
    manager.record_access(&path, "ABC123").unwrap();
    let diffs = vec![LineDiff::for_insertion(10, "new line")];
    manager.record_edit(&path, "ABC123", "DEF456", diffs).unwrap();

    // Undo
    let result = manager.undo(&path, false).unwrap();

    // Verify undo returned the inverse diff
    assert_eq!(result.diffs.len(), 1);
    assert_eq!(result.diffs[0].line_num, 10);
    assert!(result.diffs[0].new_content.is_none()); // Insertion inverse is deletion
    assert_eq!(result.current_hash, "DEF456"); // Hash after edit

    // Verify redo works (proves undo moved entry to redo stack)
    manager.redo(&path, false).unwrap();
}

#[test]
fn test_manager_redo() {
    let manager = HistoryManager::new();
    let path = std::path::PathBuf::from("/test/file.rs");

    // Setup: record access, edit, then undo
    manager.record_access(&path, "ABC123").unwrap();
    let diffs = vec![LineDiff::for_insertion(10, "new line")];
    manager.record_edit(&path, "ABC123", "DEF456", diffs).unwrap();
    manager.undo(&path, false).unwrap();

    // Redo
    let result = manager.redo(&path, false).unwrap();

    // Verify redo returned the original diffs
    assert_eq!(result.diffs.len(), 1);
    assert_eq!(result.diffs[0].new_content, Some("new line".to_string()));
    assert_eq!(result.current_hash, "DEF456"); // Hash after undo (same as after edit)

    // Verify undo works again (proves redo moved entry back to undo stack)
    manager.undo(&path, false).unwrap();
}

#[test]
fn test_manager_new_edit_clears_redo_stack() {
    let manager = HistoryManager::new();
    let path = std::path::PathBuf::from("/test/file.rs");

    // Setup: two edits, then undo first
    manager.record_access(&path, "HASH1").unwrap();
    let diffs1 = vec![LineDiff::for_insertion(10, "line 1")];
    manager.record_edit(&path, "HASH1", "HASH2", diffs1).unwrap();

    let diffs2 = vec![LineDiff::for_insertion(11, "line 2")];
    manager.record_edit(&path, "HASH2", "HASH3", diffs2).unwrap();

    // Undo twice
    manager.undo(&path, false).unwrap();
    manager.undo(&path, false).unwrap();

    // Make new edit - should clear redo stack
    manager.record_access(&path, "HASH1").unwrap();
    let diffs3 = vec![LineDiff::for_insertion(12, "line 3")];
    manager.record_edit(&path, "HASH1", "HASH4", diffs3).unwrap();

    // Verify redo is no longer available (redo stack was cleared)
    let result = manager.redo(&path, false);
    assert!(result.is_err());
}

#[test]
fn test_manager_undo_no_history() {
    let manager = HistoryManager::new();
    let path = std::path::PathBuf::from("/test/file.rs");

    let result = manager.undo(&path, false);
    assert!(result.is_err());
    match result.unwrap_err() {
        HistoryError::NoHistory => {}
        _ => panic!("Expected NoHistory error"),
    }
}

#[test]
fn test_manager_redo_no_redo() {
    let manager = HistoryManager::new();
    let path = std::path::PathBuf::from("/test/file.rs");

    let result = manager.redo(&path, false);
    assert!(result.is_err());
    match result.unwrap_err() {
        HistoryError::NoRedo => {}
        _ => panic!("Expected NoRedo error"),
    }
}

#[test]
fn test_manager_get_history() {
    let manager = HistoryManager::new();
    let path = std::path::PathBuf::from("/test/file.rs");

    // No history yet
    let result = manager.get_history(&path, None);
    assert!(result.is_empty());

    // Add some history
    manager.record_access(&path, "HASH1").unwrap();
    let diffs1 = vec![LineDiff::for_insertion(10, "line 1")];
    manager.record_edit(&path, "HASH1", "HASH2", diffs1).unwrap();

    let diffs2 = vec![LineDiff::for_insertion(11, "line 2")];
    manager.record_edit(&path, "HASH2", "HASH3", diffs2).unwrap();

    // Get all history
    let entries = manager.get_history(&path, None);
    assert_eq!(entries.len(), 2);

    // Get limited history
    let entries = manager.get_history(&path, Some(1));
    assert_eq!(entries.len(), 1);
}

#[test]
fn test_manager_max_entries_overflow() {
    let manager = HistoryManager::new();
    let path = std::path::PathBuf::from("/test/file.rs");

    // Add 60 entries (more than max of 50)
    manager.record_access(&path, "HASH0").unwrap();
    for i in 1..=60 {
        let old_hash = format!("HASH{}", i - 1);
        let new_hash = format!("HASH{}", i);
        let diffs = vec![LineDiff::for_insertion(i, &format!("line {}", i))];
        manager.record_edit(&path, &old_hash, &new_hash, diffs).unwrap();
    }

    // Should be able to undo 50 times (max entries)
    for i in 0..50 {
        let result = manager.undo(&path, false);
        assert!(result.is_ok(), "Undo {} should succeed", i);
    }

    // 51st undo should fail (no more history)
    let result = manager.undo(&path, false);
    assert!(result.is_err());
}
