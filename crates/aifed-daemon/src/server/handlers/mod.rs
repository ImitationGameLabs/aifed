//! HTTP request handlers

pub mod daemon;
pub mod history;
pub mod management;
pub mod operations;

pub use daemon::{health, heartbeat, list_servers, shutdown, status};
pub use history::{get_history, record_access, record_edit, redo, undo};
pub use management::{start_server, stop_server};
pub use operations::{
    completions, definition, diagnostics, did_change, did_close, did_open, hover, references,
    rename,
};
