# aifed

AI-first CLI text editor with safe edits and LSP-powered code operations.

This is the `aifed` binary crate. It provides:

- `read` / `edit` — file I/O with hashline verification for deterministic, conflict-free edits.
- `lsp` — LSP-powered code operations (definitions, symbols).
- `outline` — structural outline across 8+ languages via tree-sitter.
- `history` / `undo` / `redo`, `copy` / `paste` / `clipboard` — edit history and clipboard.

## Installation

Install both `aifed` and [`aifed-daemon`](https://crates.io/crates/aifed-daemon) for full functionality:

```sh
cargo install aifed
cargo install aifed-daemon
```

Without `aifed-daemon`, `read` / `edit` / `info` / `outline` still work (edit history and concurrency protections are disabled); `lsp`, `history`, `undo`, `redo`, `copy`, `paste`, `clipboard` require the daemon and will error. `aifed` spawns `aifed-daemon` from `PATH`.

## Ecosystem

- [`aifed-daemon`](https://crates.io/crates/aifed-daemon) — long-running background daemon.
- [`aifed-daemon-client`](https://crates.io/crates/aifed-daemon-client) — Rust client for the daemon IPC protocol.
- [`aifed-common`](https://crates.io/crates/aifed-common) — shared config, types, and errors.

See the [top-level README](https://github.com/ImitationGameLabs/aifed#readme) for full documentation.
