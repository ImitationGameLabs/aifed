//! HTTP request handlers

pub mod daemon;
pub mod management;
pub mod operations;

pub use daemon::{health, list_servers, status};
pub use management::{start_server, stop_server};
pub use operations::{
    completions, definition, diagnostics, did_change, did_close, did_open, hover, references,
    rename,
};
