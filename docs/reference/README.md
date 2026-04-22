# aifed Reference Documentation

This directory contains reference documentation for all **aifed** commands and concepts.

## Quick Navigation

| Category               | Description                          | Reference                            | Status      |
| ---------------------- | ------------------------------------ | ------------------------------------ | ----------- |
| **CLI Overview**       | Global options, workspace detection  | [cli-overview.md](cli-overview.md)   | Implemented |
| **Locator**            | Positioning mechanism for safe edits | [locator.md](locator.md)             | Implemented |
| **Edit Commands**      | Unified edit with + and - operators   | [edit-commands.md](edit-commands.md) | Implemented |
| **Read Commands**      | info, read                           | [read-commands.md](read-commands.md) | Implemented |
| **LSP Integration**    | diag, symbols, rename, etc.          | [lsp.md](lsp.md)                     | Implemented |
| **History & Recovery** | history, undo, redo                  | [history.md](history.md)             | Implemented |
| **Clipboard**          | copy, paste, clipboard               | [clipboard.md](clipboard.md)         | Implemented |
| **Configuration**      | config command and file format       | [configuration.md](configuration.md) | Planned     |
| **Utilities**          | diff, format                         | [utilities.md](utilities.md)         | Planned     |

## Command Categories

### Core Editing
- [`edit`](edit-commands.md#edit) - Edit file content (delete and insert; replacement via `-` + `+`)

### File Information
- [`read`](read-commands.md#read) - Read file content
- [`info`](read-commands.md#info) - Get file metadata and stats

### LSP Integration
- [`lsp diag`](lsp.md#lsp-diag) - Get errors and warnings
- [`lsp symbols`](lsp.md#lsp-symbols) - List symbols in file
- [`lsp rename`](lsp.md#lsp-rename) - Rename symbol across references
- [`lsp refs`](lsp.md#lsp-refs) - Find all references
- [`lsp def`](lsp.md#lsp-def) - Go to definition
- [`lsp hover`](lsp.md#lsp-hover) - Get type information
- [`lsp complete`](lsp.md#lsp-complete) - Get completions

### History & Recovery *(Implemented)*
- [`history`](history.md#history) - View edit history
- [`undo`](history.md#undo) - Undo recent edits
- [`redo`](history.md#redo) - Redo undone edits

### Clipboard *(Implemented)*
- [`copy`](clipboard.md#copy) - Copy lines to clipboard
- [`paste`](clipboard.md#paste) - Paste clipboard content to file
- [`clipboard`](clipboard.md#clipboard) - Show clipboard content

### Configuration *(Planned)*
- [`config`](configuration.md#config) - Manage configuration
- [`init`](configuration.md#init) - Initialize project

### Utilities *(Planned)*
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
- Clear error messages for easy parsing

## Document Conventions

This documentation follows consistent conventions for clarity:

### Hash Format

All examples use **2-character base32hex hashes** (e.g., `AB`, `3K`):

```
main.rs 42:AB
```

The hash encodes 10 bits of the xxHash64 output using base32hex character set (`0-9`, `A-V`). See [CLI Design Notes](../cli-design-notes.md#2-hash-algorithm) for the algorithm details.

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
