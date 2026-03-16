mod cli;
mod commands;
mod error;
mod hash;
mod locator;
mod output;

use crate::cli::{Cli, Commands};
use crate::output::{OutputFormat, format_error};

fn main() {
    let args = Cli::parse_args();
    let format = if args.json { OutputFormat::Json } else { OutputFormat::Text };

    let result = match args.command {
        Commands::Read { file, locator, no_hashes, context } => {
            commands::read(&file, locator.as_deref(), no_hashes, context, format)
        }
        Commands::Info { file } => commands::info(&file, format),
        Commands::Edit { file, operation, locator, content, dry_run } => {
            commands::edit(&file, &operation, &locator, content.as_deref(), dry_run, format)
        }
    };

    if let Err(e) = result {
        eprintln!("{}", format_error(&e, format));
        std::process::exit(1);
    }
}
