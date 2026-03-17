# AGENTS.md

AI Agent working guide. This document provides code structure and decision rules for AI agents.

## Directory Structure

```
.
├── flake.nix                  # Flake entry point
├── crates/                    # Rust workspace members
├── docs/                      # Project documentation
│   ├── agent-action-wizards/ # Step-by-step guides for AI agents
│   └── reference/            # CLI command reference
└── nix/
      ├── common.nix   # Core config (crate paths, dependencies)
      ├── packages.nix # Flake output packages
      └── dev/
            ├── shell.nix  # Development environment
            └── checks.nix # CI checks
```

## Common Tasks

- [Add workspace member](docs/agent-action-wizards/add-workspace-member.md)
- [CLI test walkthrough](docs/agent-action-wizards/aifed-cli-test-walkthrough.md)

## Testing aifed Commands

When testing aifed commands, use the `.playground/` directory:

```bash
cd .playground
../target/debug/aifed <command>
```

The `.playground/` directory is in `.gitignore`. Refer to the `CLI test walkthrough` for test cases.

## Dependency Management

When adding dependencies to any crate:
1. Add to `[workspace.dependencies]` in root `Cargo.toml`
2. Reference in crate's `Cargo.toml` with `workspace = true`

Example:
```toml
# Root Cargo.toml
[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }

# crates/my-app/Cargo.toml
[dependencies]
serde = { workspace = true }
```

## Verification Checklist

After modifying Nix files:
- `nixfmt <nix file>` - Format single file
- `nixfmt $(find nix/ -name "*.nix") flake.nix` - Format all Nix files at once
- `statix check .` - Static analysis (run from project root)

After modifying Rust code:
- `cargo clippy` - Lint check
- `cargo fmt` - Format check
- `cargo test` - Run tests
