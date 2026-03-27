use std::path::Path;

use crate::error::{Error, Result};
use crate::output::{FileInfo, OutputFormat, format_file_info};

/// Execute the info command
pub fn execute(path: &Path, format: OutputFormat) -> Result<()> {
    if !path.exists() {
        return Err(Error::FileNotFound {
            path: crate::file::to_absolute(path),
            cwd: std::env::current_dir().unwrap_or_default(),
        });
    }

    let metadata = std::fs::metadata(path)
        .map_err(|e| Error::InvalidIo { path: path.to_path_buf(), source: e })?;

    let content = crate::file::read_text_file(path)?;

    let lines = crate::file::split_lines(&content).len();
    let size = metadata.len();

    let info = FileInfo { path: path.display().to_string(), lines, size };

    println!("{}", format_file_info(&info, format));
    Ok(())
}
