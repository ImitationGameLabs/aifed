use std::path::Path;

use aifed_common::load_registry_for_path;

use crate::error::{Error, Result};
use crate::file::{read_text_file, to_absolute};
use crate::language::LanguageResolver;
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
    // Honor `[[language]]` overlays when config is available; fall back to
    // grammar defaults on any load error so outline stays usable with zero or
    // corrupt config.
    let registry = match load_registry_for_path(path) {
        Ok(r) => LanguageResolver::with_overlays(r.language_overlays()),
        Err(e) => {
            eprintln!("Warning: config load failed, using grammar defaults: {e}");
            LanguageResolver::from_defaults()
        }
    };
    let outline = crate::outline::extract(path, &content, imports, &registry)?;
    println!("{}", format_outline(&outline, format));
    Ok(())
}
