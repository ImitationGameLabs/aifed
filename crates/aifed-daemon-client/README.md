# aifed-daemon-client

Rust client for the [aifed-daemon](https://crates.io/crates/aifed-daemon) IPC protocol (Unix-socket JSON).

This library crate is used by [`aifed`](https://crates.io/crates/aifed) to talk to the daemon. It provides the typed request/response types and an async HTTP-over-unix-socket client.

## Ecosystem

- [`aifed`](https://crates.io/crates/aifed) — the editor binary.
- [`aifed-daemon`](https://crates.io/crates/aifed-daemon) — the daemon this client targets.
- [`aifed-common`](https://crates.io/crates/aifed-common) — shared config, types, and errors.

See the [top-level README](https://github.com/ImitationGameLabs/aifed#readme) for full documentation.
