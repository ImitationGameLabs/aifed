# aifed Reference Documentation

This directory contains reference documentation for all **aifed** commands and concepts.

## Quick Navigation

| Category                | Description                                       | Reference                            |
| ----------------------- | ------------------------------------------------- | ------------------------------------ |
| **CLI Overview**        | Global options, environment variables, exit codes | [cli-overview.md](cli-overview.md)   |
| **Locator**             | Positioning mechanism for safe edits              | [locator.md](locator.md)             |
| **Edit Commands**       | Unified edit with ~, +, - operators               | [edit-commands.md](edit-commands.md) |
| **Read Commands**       | info, read                                        | [read-commands.md](read-commands.md) |
| **LSP Integration**     | diagnostics, symbols, rename, etc.                | [lsp.md](lsp.md)                     |
| **History & Snapshots** | snapshot, history, undo                           | [history.md](history.md)             |
| **Configuration**       | config command and file format                    | [configuration.md](configuration.md) |
| **Utilities**           | diff, format                                      | [utilities.md](utilities.md)         |

## Command Categories

### Core Editing
- [`edit`](edit-commands.md#edit) - Edit file content (replace, insert, delete)

### File Information
- [`read`](read-commands.md#read) - Read file content
- [`info`](read-commands.md#info) - Get file metadata and stats

### LSP Integration
- [`diagnostics`](lsp.md#diagnostics) - Get errors and warnings
- [`symbols`](lsp.md#symbols) - List symbols in file
- [`rename`](lsp.md#rename) - Rename symbol across references
- [`references`](lsp.md#references) - Find all references
- [`definition`](lsp.md#definition) - Go to definition
- [`hover`](lsp.md#hover) - Get type information

### History & Snapshots
- [`snapshot`](history.md#snapshot) - Manage file snapshots
- [`history`](history.md#history) - View edit history
- [`undo`](history.md#undo) - Undo recent edits

### Configuration
- [`config`](configuration.md#config) - Manage configuration
- [`init`](configuration.md#init) - Initialize project

### Utilities
- [`diff`](utilities.md#diff) - Show file differences
- [`format`](utilities.md#format) - Format files

## Key Concepts

### Locator

The locator is aifed's positioning mechanism for safe, deterministic edits. The primary format is **hashline**, which combines line numbers with content hashes to ensure edits are applied at the correct location.

See [locator.md](locator.md) for detailed documentation.

### AI-First Design

aifed is designed specifically for AI agents:
- Long flags only (`--file` not `-f`) - clearer, self-documenting
- Explicit arguments - no interactive prompts
- Structured JSON output with `--json`
- Machine-parseable errors with exit codes

## Document Conventions

This documentation follows consistent conventions for clarity:

### Hash Length

All examples use **6-character hashes** (e.g., `abc123`, `def456`):

```
main.rs 42:abc123
```

**Note:** This is a provisional standard for documentation consistency. The actual hash algorithm and length are TBD - see [CLI Design Notes](../cli-design-notes.md#2-hash-algorithm) for options being considered.

### Example Language

All code examples use **Rust** syntax:

```rust
fn main() {
    println!("hello");
}
```

**Why Rust?**

- aifed is written in Rust, making it the natural choice for consistency
- Using a single language across all documentation keeps examples uniform
- The concepts demonstrated are language-agnostic - Rust is simply the vehicle

The tool itself supports any programming language with appropriate LSP servers configured.

## See Also

- [CLI Design Notes](../cli-design-notes.md) - Design rationale and trade-offs
- [Project README](../../README.md) - Project overview
