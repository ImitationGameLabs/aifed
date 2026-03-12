# Add Workspace Member Wizard

This document guides AI to add a new workspace member (crate) to the project.

## Instructions for AI

Follow the steps below in order. For each step, **check first** before taking action.

---

## Step 0: Check for Placeholder Crate

Check if a placeholder `hello` crate exists:

```bash
ls -d crates/hello
```

- If exists → Ask user: "Found placeholder `hello` crate. Rename it to `<name>` instead of creating new?"
  - If yes → Rename `crates/hello` to `crates/<name>`, then proceed to Step 3
  - If no → Proceed to Step 1
- If not exists → Proceed to Step 1

---

## Step 1: Check if Crate Already Exists

Check if `crates/<name>/` directory exists:

```bash
ls -d crates/<name>
```

- If exists → Skip to Step 3
- If not exists → Proceed to Step 2

---

## Step 2: Create Crate with cargo new

Use `cargo new` to create the crate:

**For binary crate:**

```bash
cargo new crates/<name>
```

**For library crate:**

```bash
cargo new crates/<name> --lib
```

This automatically creates:
- `crates/<name>/Cargo.toml`
- `crates/<name>/src/main.rs` (binary) or `crates/<name>/src/lib.rs` (library)

---

## Step 3: Check Root Cargo.toml

Check if the crate is registered in workspace members:

```bash
grep -q 'crates/<name>' Cargo.toml
```

- If found → Skip to Step 4
- If not found → Add to workspace members:

```toml
[workspace]
members = [
  # ... existing members ...
  "crates/<name>",
]
```

---

## Step 4: Check Nix Registration

Check `nix/common.nix` for the crate entry:

**For binary crate** - check `binaryCratePaths`:

```bash
grep -q '<name>.*=.*"crates/<name>"' nix/common.nix
```

- If found → Skip to Step 5
- If not found → Add to `binaryCratePaths`:

```nix
binaryCratePaths = mapToAbsolute {
  # ... existing entries ...
  <name> = "crates/<name>";
};
```

**For library crate** - check `libraryCratePaths`:

```bash
grep -q '<name>.*=.*"crates/<name>"' nix/common.nix
```

- If found → Skip to Step 5
- If not found → Add to `libraryCratePaths`:

```nix
libraryCratePaths = mapToAbsolute {
  # ... existing entries ...
  <name> = "crates/<name>";
};
```

---

## Step 5: (Optional) Set as Default Package

For binary crates that should be the main package, check and update `nix/packages.nix`:

```nix
default = packages.<name>;
```

---

## Step 6: (Optional) Add Native Dependencies

If the crate requires system libraries (e.g., openssl, sqlite), add to `nix/common.nix`:

```nix
commonArgs = {
  # ...
  buildInputs = with pkgs; [
    openssl
    # Add other system libraries here
  ];
};
```

---

## Quick Reference

| Crate Type | cargo new flag | Nix Registry | Generates Package |
| ---------- | -------------- | ------------ | ----------------- |
| Binary     | (default)      | `binaryCratePaths` | Yes |
| Library    | `--lib`        | `libraryCratePaths` | No (dep tracking only) |

## Verification

After all steps:

1. Format Nix files:
   - Single file: `nixfmt nix/common.nix`
   - All at once: `nixfmt $(find nix/ -name "*.nix") flake.nix`
2. Static analysis: `statix check .` (run from project root)
3. Build verification: `nix build .#<name>` (for binary crates)
4. Cargo check: `cargo check -p <name>`
