# aifed-daemon

Long-running background daemon for [aifed](https://crates.io/crates/aifed): edit-history, concurrency tracking, and a local code index served over a Unix socket.

This crate provides the `aifed-daemon` binary. It is spawned by `aifed` from `PATH`; you normally install it alongside `aifed` rather than run it directly.

## Installation

```sh
cargo install aifed-daemon
cargo install aifed # the editor that uses this daemon
```

## Ecosystem

- [`aifed`](https://crates.io/crates/aifed) — the editor binary.
- [`aifed-daemon-client`](https://crates.io/crates/aifed-daemon-client) — Rust client for this daemon's IPC protocol.
- [`aifed-common`](https://crates.io/crates/aifed-common) — shared config, types, and errors.

See the [top-level README](https://github.com/ImitationGameLabs/aifed#readme) for full documentation.
