//! Common types for aifed-daemon and aifed-daemon-client
//!
//! This crate contains API request/response types shared between:
//! - `aifed-daemon`: The daemon server
//! - `aifed-daemon-client`: The client library
//! - `aifed`: The CLI (via client)

mod error;
mod socket;
mod types;

pub use error::*;
pub use socket::*;
pub use types::*;
