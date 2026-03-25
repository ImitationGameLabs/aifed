use std::path::Path;

use crate::error::{Error, Result};
use crate::output::{FileInfo, OutputFormat, format_file_info};

/// Execute the info command
pub fn execute(path: &Path, format: OutputFormat) -> Result<()> {
    if !path.exists() {
        return Err(Error::FileNotFound { path: path.to_path_buf() });
    }

    let metadata = std::fs::metadata(path)
        .map_err(|e| Error::InvalidIo { path: path.to_path_buf(), source: e })?;

    let content = std::fs::read_to_string(path)
        .map_err(|e| Error::InvalidIo { path: path.to_path_buf(), source: e })?;

    let lines = crate::file::split_lines(&content).len();
    let size = metadata.len();

    let info = FileInfo { path: path.display().to_string(), lines, size };

    println!("{}", format_file_info(&info, format));
    Ok(())
}
