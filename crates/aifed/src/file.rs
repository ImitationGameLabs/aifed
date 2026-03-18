//! File I/O utilities for aifed.

use std::path::Path;

use crate::error::{Error, Result};

/// Write lines to a file, optionally preserving trailing newline.
pub fn write_file(path: &Path, lines: &[String], trailing_newline: bool) -> Result<()> {
    let content = lines.join("\n");
    let content = if trailing_newline { content + "\n" } else { content };
    std::fs::write(path, content)
        .map_err(|e| Error::InvalidIo { path: path.to_path_buf(), source: e })?;
    Ok(())
}
