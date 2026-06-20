//! Common types for aifed-daemon and aifed-daemon-client
//!
//! This crate contains API request/response types shared between:
//! - `aifed-daemon`: The daemon server
//! - `aifed-daemon-client`: The client library
//! - `aifed`: The CLI (via client)

pub mod config;
mod error;
mod socket;
mod types;
pub mod workspace;

pub use config::{
    ConfigError, IndentConfig, IndentStyleConfig, LanguageConfig, LspServerConfig, Registry,
    ensure_default_config, global_config_path, load_registry_for_path, load_registry_for_workspace,
    normalize_extension, normalize_language,
};
pub use error::*;
pub use socket::*;
pub use types::*;
pub use workspace::{Workspace, WorkspaceError, detect_workspace};
