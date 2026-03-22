//! History tracking for file edits

mod entry;
mod manager;

pub use entry::{HistoryEntry, LineDiff};
pub use manager::{HistoryError, HistoryManager};

// Re-export for tests
#[cfg(test)]
pub use manager::FileHistory;

#[cfg(test)]
mod tests;
