# aifed Reference Documentation

This directory contains reference documentation for all **aifed** commands and concepts.

## Quick Navigation

| Category                | Description                                       | Reference                                |
| ----------------------- | ------------------------------------------------- | ---------------------------------------- |
| **CLI Overview**        | Global options, environment variables, exit codes | [cli-overview.md](cli-overview.md)       |
| **Locator**             | Positioning mechanism for safe edits              | [locator.md](locator.md)                 |
| **Edit Commands**       | replace, insert, delete, edit                     | [edit-commands.md](edit-commands.md)     |
| **File Operations**     | info, read                                        | [file-operations.md](file-operations.md) |
| **LSP Integration**     | diagnostics, symbols, rename, etc.                | [lsp.md](lsp.md)                         |
| **History & Snapshots** | snapshot, history, undo                           | [history.md](history.md)                 |
| **Configuration**       | config command and file format                    | [configuration.md](configuration.md)     |
| **Utilities**           | diff, format                                      | [utilities.md](utilities.md)             |

## Command Categories

### Core Editing
- [`replace`](edit-commands.md#replace) - Replace content at specified location
- [`insert`](edit-commands.md#insert) - Insert new content
- [`delete`](edit-commands.md#delete) - Delete content
- [`edit`](edit-commands.md#edit) - Atomic batch operations

### File Information
- [`info`](file-operations.md#info) - Get file metadata and hashes
- [`read`](file-operations.md#read) - Read file content

### LSP Integration
- [`diagnostics`](lsp.md#diagnostics) - Get errors and warnings
- [`symbols`](lsp.md#symbols) - List symbols in file
- [`rename`](lsp.md#rename) - Rename symbol across references
- [`references`](lsp.md#references) - Find all references
- [`definition`](lsp.md#definition) - Go to definition
- [`hover`](lsp.md#hover) - Get type information
- [`organize-imports`](lsp.md#organize-imports) - Organize imports

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

## See Also

- [CLI Design Notes](../cli-design-notes.md) - Design rationale and trade-offs
- [Project README](../../README.md) - Project overview
