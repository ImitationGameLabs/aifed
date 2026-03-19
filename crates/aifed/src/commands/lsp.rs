//! LSP operation commands

use crate::args::LspCommands;
use crate::error::{Error, Result};
use crate::hash::hash_line;
use crate::locator::{Locator, SymbolLocator};
use crate::output::{self, OutputFormat};
use aifed_common::{
    DiagnosticsRequest, HoverRequest, LspPositionRequest, Position, RenameRequest, socket_path,
};
use aifed_daemon_client::DaemonClient;
use std::io::BufRead;
use std::path::Path;

/// Detect language from file extension
fn detect_language(file: &Path) -> Result<String> {
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");

    let lang = match ext {
        "rs" => "rust",
        "py" => "python",
        "ts" => "typescript",
        "tsx" => "typescriptreact",
        "js" => "javascript",
        "jsx" => "javascriptreact",
        "go" => "go",
        "java" => "java",
        "c" => "c",
        "cpp" | "cc" | "cxx" => "cpp",
        "h" => "c",
        "hpp" => "cpp",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "kt" => "kotlin",
        "scala" => "scala",
        "lua" => "lua",
        _ => return Err(Error::Lsp { message: format!("Unknown file extension: {}", ext) }),
    };

    Ok(lang.to_string())
}

/// Get client connected to the daemon for the given workspace
fn get_client(socket: Option<&Path>) -> Result<(DaemonClient, std::path::PathBuf)> {
    let socket = match socket {
        Some(p) => p.to_path_buf(),
        None => {
            let cwd = std::env::current_dir()
                .map_err(|e| Error::InvalidIo { path: Path::new(".").to_path_buf(), source: e })?;
            socket_path(&cwd)?
        }
    };

    Ok((DaemonClient::new(&socket), socket))
}

/// Read a specific line from a file (1-based line number)
fn read_line(file: &Path, line_num: usize) -> Result<String> {
    let f = std::fs::File::open(file)
        .map_err(|e| Error::InvalidIo { path: file.to_path_buf(), source: e })?;
    let reader = std::io::BufReader::new(f);

    reader
        .lines()
        .nth(line_num - 1)
        .ok_or_else(|| Error::InvalidLocator {
            input: line_num.to_string(),
            reason: "Line number out of range".to_string(),
        })?
        .map_err(|e| Error::InvalidIo { path: file.to_path_buf(), source: e })
}

/// Extract identifiers from a line, returning (index, name, offset) tuples
fn extract_symbols(line: &str) -> Vec<(u32, String, u32)> {
    let mut symbols = Vec::new();
    let mut count = 0u32;
    let mut pos = 0usize;

    while pos < line.len() {
        // Skip non-identifier characters
        while pos < line.len() && !line[pos..].starts_with(|c: char| c.is_alphabetic() || c == '_')
        {
            pos += line[pos..].char_indices().next().map(|(_, c)| c.len_utf8()).unwrap_or(1);
        }

        if pos >= line.len() {
            break;
        }

        // Find end of identifier
        let start = pos;
        while pos < line.len() {
            let ch = line[pos..].chars().next().unwrap();
            if ch.is_alphanumeric() || ch == '_' || ch == '!' || ch == '?' {
                pos += ch.len_utf8();
            } else {
                break;
            }
        }

        let ident = &line[start..pos];
        count += 1;
        symbols.push((count, ident.to_string(), start as u32));
    }

    symbols
}

/// Resolve position from hashline and symbol locator
fn resolve_position(file: &Path, hashline: &str, symbol: &str) -> Result<(u32, u32)> {
    // Parse hashline
    let locator = Locator::parse(hashline)?;
    let line_num = locator.line().ok_or_else(|| Error::InvalidLocator {
        input: hashline.to_string(),
        reason: "Virtual line (0:00) not supported for LSP operations".to_string(),
    })?;

    // Read line
    let line_content = read_line(file, line_num)?;

    // Verify hash if hashline was provided
    if let Locator::Hashline { hash, .. } = &locator {
        let actual_hash = hash_line(&line_content);
        if actual_hash != *hash {
            return Err(Error::HashMismatch {
                path: file.to_path_buf(),
                line: line_num,
                expected: hash.clone(),
                actual: actual_hash,
                actual_content: line_content,
            });
        }
    }

    // Parse symbol locator
    let sym_loc = SymbolLocator::parse(symbol)?;

    // Find character offset
    let char_offset = sym_loc.find_offset(&line_content).ok_or_else(|| Error::InvalidLocator {
        input: symbol.to_string(),
        reason: format!(
            "Symbol '{}' not found at index {} on line {}",
            sym_loc.name, sym_loc.index, line_num
        ),
    })?;

    Ok((line_num as u32, char_offset + 1)) // Convert to 1-based
}

pub async fn execute(cmd: &LspCommands, socket: Option<&Path>, format: OutputFormat) -> Result<()> {
    // Symbols command doesn't need daemon - it just reads the file
    if let LspCommands::Symbols { file, locator } = cmd {
        let loc = Locator::parse(locator)?;

        match loc {
            Locator::Line(line) | Locator::Hashline { line, .. } => {
                let line_content = read_line(file, line)?;
                let hash = hash_line(&line_content);

                // Print hashline
                println!("{}:{}|{}", line, hash, line_content);

                // Print symbol locators
                for (idx, name, _offset) in extract_symbols(&line_content) {
                    println!("S{}:{}", idx, name);
                }
            }
            Locator::LineRange { start, end } => {
                for line_num in start..=end {
                    let line_content = read_line(file, line_num)?;
                    let hash = hash_line(&line_content);

                    println!("{}:{}|{}", line_num, hash, line_content);

                    for (idx, name, _offset) in extract_symbols(&line_content) {
                        println!("S{}:{}", idx, name);
                    }
                }
            }
        }

        return Ok(());
    }

    // Other LSP commands need the daemon
    let (client, socket_path) = get_client(socket)?;

    // Check if daemon is running
    if !client.is_running().await {
        let workspace = socket_path.parent().unwrap_or(&socket_path);
        return Err(Error::DaemonNotRunning { workspace: workspace.to_path_buf() });
    }

    match cmd {
        LspCommands::Diag { file } => {
            let language = detect_language(file)?;
            let file_path = file.to_string_lossy().to_string();
            let request = DiagnosticsRequest { language, file_path };

            let response = client.diagnostics(request).await?;
            println!("{}", output::format_diagnostics_response(&response, format));
            Ok(())
        }

        LspCommands::Hover { file, hashline, symbol } => {
            let (line, col) = resolve_position(file, hashline, symbol)?;
            let language = detect_language(file)?;
            let file_path = file.to_string_lossy().to_string();
            let request = HoverRequest {
                language,
                file_path,
                position: Position { line: line - 1, character: col - 1 },
            };

            let response = client.hover(request).await?;
            println!("{}", output::format_hover_response(&response, format));
            Ok(())
        }

        LspCommands::Def { file, hashline, symbol } => {
            let (line, col) = resolve_position(file, hashline, symbol)?;
            let language = detect_language(file)?;
            let file_path = file.to_string_lossy().to_string();
            let request = LspPositionRequest {
                language,
                file_path,
                position: Position { line: line - 1, character: col - 1 },
            };

            let response = client.goto_definition(request).await?;
            println!("{}", output::format_definition_response(&response, format));
            Ok(())
        }

        LspCommands::Refs { file, hashline, symbol } => {
            let (line, col) = resolve_position(file, hashline, symbol)?;
            let language = detect_language(file)?;
            let file_path = file.to_string_lossy().to_string();
            let request = LspPositionRequest {
                language,
                file_path,
                position: Position { line: line - 1, character: col - 1 },
            };

            let response = client.references(request).await?;
            println!("{}", output::format_references_response(&response, format));
            Ok(())
        }

        LspCommands::Complete { file, hashline, symbol } => {
            let (line, col) = resolve_position(file, hashline, symbol)?;
            let language = detect_language(file)?;
            let file_path = file.to_string_lossy().to_string();
            let request = LspPositionRequest {
                language,
                file_path,
                position: Position { line: line - 1, character: col - 1 },
            };

            let response = client.completions(request).await?;
            println!("{}", output::format_completions_response(&response, format));
            Ok(())
        }

        LspCommands::Rename { file, hashline, symbol, new_name } => {
            let (line, col) = resolve_position(file, hashline, symbol)?;
            let language = detect_language(file)?;
            let file_path = file.to_string_lossy().to_string();
            let request = RenameRequest {
                language,
                file_path,
                position: Position { line: line - 1, character: col - 1 },
                new_name: new_name.clone(),
            };

            let response = client.rename(request).await?;
            println!("{}", output::format_rename_response(&response, format));
            Ok(())
        }

        // Symbols is handled above with early return, this arm is unreachable
        LspCommands::Symbols { .. } => unreachable!(),
    }
}
