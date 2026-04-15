//! LSP operation commands

use crate::args::LspCommands;
use crate::error::{Error, Result};
use crate::hash::{hash_file, hash_line};
use crate::locator::{Locator, SymbolLocator};
use crate::output::{self, OutputFormat, RenameFileDiff};
use aifed_common::{
    DiagnosticsRequest, HoverRequest, LineDiffDto, LspPositionRequest, Position, RenameRequest,
};
use aifed_daemon_client::DaemonClient;
use std::io::BufRead;
use std::path::{Path, PathBuf};

/// Convert path to absolute path for LSP requests.
/// LSP requires absolute paths for file:// URIs.
fn canonicalize_path(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .map_err(|e| Error::InvalidIo { path: path.to_path_buf(), source: e })
}

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
            pos += line[pos..]
                .char_indices()
                .next()
                .map(|(_, c)| c.len_utf8())
                .unwrap_or(1);
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

/// Compute line diffs for rename operations by comparing original and new content.
/// For rename operations, we simply compare lines at the same positions.
fn compute_rename_diffs(original_lines: &[&str], new_lines: &[&str]) -> Vec<LineDiffDto> {
    let mut diffs = Vec::new();
    let max_lines = original_lines.len().max(new_lines.len());

    for i in 0..max_lines {
        let line_num = i + 1; // 1-based
        let orig = original_lines.get(i);
        let new = new_lines.get(i);

        match (orig, new) {
            (Some(&old), Some(&new)) => {
                if old != new {
                    // Line was modified
                    diffs.push(LineDiffDto {
                        line_num,
                        old_hash: None,
                        old_content: Some(old.to_string()),
                        new_content: Some(new.to_string()),
                    });
                }
            }
            (Some(&old), None) => {
                // Line was deleted
                diffs.push(LineDiffDto {
                    line_num,
                    old_hash: None,
                    old_content: Some(old.to_string()),
                    new_content: None,
                });
            }
            (None, Some(&new)) => {
                // Line was inserted
                diffs.push(LineDiffDto {
                    line_num,
                    old_hash: None,
                    old_content: None,
                    new_content: Some(new.to_string()),
                });
            }
            (None, None) => {}
        }
    }

    diffs
}

struct PreparedRenameFile {
    path: PathBuf,
    file_path: String,
    edit_count: usize,
    expected_hash: String,
    new_hash: String,
    new_content: String,
    diffs: Vec<LineDiffDto>,
    new_lines: Vec<String>,
}

impl PreparedRenameFile {
    fn to_output_diff(&self) -> RenameFileDiff {
        RenameFileDiff {
            file_path: self.file_path.clone(),
            edit_count: self.edit_count,
            diffs: self.diffs.clone(),
            new_lines: self.new_lines.clone(),
        }
    }
}

fn prepare_rename_file(file_edit: &aifed_common::FileEdit) -> Result<PreparedRenameFile> {
    let path = PathBuf::from(&file_edit.file_path);
    let content = crate::file::read_text_file(&path)?;
    let expected_hash = hash_file(content.as_bytes());
    let original_lines = crate::file::split_lines(&content);

    let new_content = crate::text_edit::apply_edits(&content, file_edit.edits.clone())?;
    let new_hash = hash_file(new_content.as_bytes());
    let new_lines = crate::file::split_lines_owned(&new_content);
    let updated_lines = crate::file::split_lines(&new_content);
    let diffs = compute_rename_diffs(&original_lines, &updated_lines);

    Ok(PreparedRenameFile {
        path,
        file_path: file_edit.file_path.clone(),
        edit_count: file_edit.edits.len(),
        expected_hash,
        new_hash,
        new_content,
        diffs,
        new_lines,
    })
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
    let char_offset = sym_loc
        .find_offset(&line_content)
        .ok_or_else(|| Error::InvalidLocator {
            input: symbol.to_string(),
            reason: format!(
                "Symbol '{}' not found at index {} on line {}",
                sym_loc.name, sym_loc.index, line_num
            ),
        })?;

    Ok((line_num as u32, char_offset + 1)) // Convert to 1-based
}

pub async fn execute(cmd: &LspCommands, client: &DaemonClient, format: OutputFormat) -> Result<()> {
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
            Locator::LineRange { start, end } | Locator::HashlineRange { start, end, .. } => {
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

    // Other LSP commands need the daemon - check if running
    if !client.is_running().await {
        return Err(Error::DaemonNotRunning {
            workspace: std::env::current_dir().unwrap_or_default().to_path_buf(),
        });
    }

    match cmd {
        LspCommands::Diag { file } => {
            let abs_path = canonicalize_path(file)?;
            let language = detect_language(&abs_path)?;
            let file_path = abs_path.to_string_lossy().to_string();
            let request = DiagnosticsRequest { language, file_path };

            let response = client.diagnostics(request).await?;
            println!("{}", output::format_diagnostics_response(&response, format));
            Ok(())
        }

        LspCommands::Hover { file, hashline, symbol } => {
            let abs_path = canonicalize_path(file)?;
            let (line, col) = resolve_position(&abs_path, hashline, symbol)?;
            let language = detect_language(&abs_path)?;
            let file_path = abs_path.to_string_lossy().to_string();
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
            let abs_path = canonicalize_path(file)?;
            let (line, col) = resolve_position(&abs_path, hashline, symbol)?;
            let language = detect_language(&abs_path)?;
            let file_path = abs_path.to_string_lossy().to_string();
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
            let abs_path = canonicalize_path(file)?;
            let (line, col) = resolve_position(&abs_path, hashline, symbol)?;
            let language = detect_language(&abs_path)?;
            let file_path = abs_path.to_string_lossy().to_string();
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
            let abs_path = canonicalize_path(file)?;
            let (line, col) = resolve_position(&abs_path, hashline, symbol)?;
            let language = detect_language(&abs_path)?;
            let file_path = abs_path.to_string_lossy().to_string();
            let request = LspPositionRequest {
                language,
                file_path,
                position: Position { line: line - 1, character: col - 1 },
            };

            let response = client.completions(request).await?;
            println!("{}", output::format_completions_response(&response, format));
            Ok(())
        }

        LspCommands::Rename { file, hashline, symbol, new_name, dry_run } => {
            let abs_path = canonicalize_path(file)?;
            let (line, col) = resolve_position(&abs_path, hashline, symbol)?;
            let language = detect_language(&abs_path)?;
            let file_path = abs_path.to_string_lossy().to_string();
            let request = RenameRequest {
                language,
                file_path,
                position: Position { line: line - 1, character: col - 1 },
                new_name: new_name.clone(),
            };

            let response = client.rename(request).await?;
            let prepared_files: Vec<PreparedRenameFile> = response
                .changes
                .iter()
                .map(prepare_rename_file)
                .collect::<Result<_>>()?;
            let rename_diffs: Vec<RenameFileDiff> = prepared_files
                .iter()
                .map(PreparedRenameFile::to_output_diff)
                .collect();

            if *dry_run {
                println!(
                    "{}",
                    output::format_rename_preview(&response, &rename_diffs, format)
                );
            } else {
                for prepared in &prepared_files {
                    std::fs::write(&prepared.path, &prepared.new_content)
                        .map_err(|e| Error::InvalidIo { path: prepared.path.clone(), source: e })?;

                    let file_str = prepared.path.to_string_lossy().to_string();
                    let expected_hash = prepared.expected_hash.clone();
                    let new_hash = prepared.new_hash.clone();
                    let diffs = prepared.diffs.clone();
                    let daemon_client = client.clone();
                    tokio::spawn(async move {
                        let _ = daemon_client
                            .record_edit(&file_str, &expected_hash, &new_hash, diffs)
                            .await;
                    });
                }
                println!(
                    "{}",
                    output::format_rename_result(&response, &rename_diffs, format)
                );
            }
            Ok(())
        }

        // Symbols is handled above with early return, this arm is unreachable
        LspCommands::Symbols { .. } => unreachable!(),
    }
}
