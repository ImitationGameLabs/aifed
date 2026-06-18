use std::path::Path;

use crate::error::{Error, Result};
use crate::file::{read_text_file, to_absolute};
use crate::output::{OutputFormat, format_outline};

/// Execute the outline command.
pub fn execute(path: &Path, imports: bool, format: OutputFormat) -> Result<()> {
    if !path.exists() {
        return Err(Error::FileNotFound {
            path: to_absolute(path),
            cwd: std::env::current_dir().unwrap_or_default(),
        });
    }

    let content = read_text_file(path)?;
    let outline = crate::outline::extract(path, &content, imports)?;
    println!("{}", format_outline(&outline, format));
    Ok(())
}
