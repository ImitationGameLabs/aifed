# History & Recovery

> **Status: Implemented** - History (in-memory, per-file), Undo/Redo

Commands for managing file edit history and recovery.

## Overview

aifed automatically tracks every edit operation, enabling undo/redo functionality for recovery.

---

## `history` - View Edit History

View edit history for a file.

### Usage

```
aifed history <FILE> [--count <N>] [--stat]
```

### Arguments

| Argument | Description      |
| -------- | ---------------- |
| `<FILE>` | Path to the file |

### Options

| Option        | Description                     |
| ------------- | ------------------------------- |
| `--count <N>` | Show last N edits [default: 10] |
| `--stat`      | Show summaries only (no diffs)  |

### Examples

```bash
# View recent history
aifed history main.rs

# Show last 20 edits
aifed history main.rs --count 20

# Show summaries only
aifed history main.rs --stat

# JSON output
aifed history main.rs --json
```

### Output Format

Each history entry shows:
- Entry ID (for reference)
- Timestamp
- Summary of changes
- Line diffs (unless `--stat` is used)

---

## `undo` - Undo Recent Edits

Undo the last edit for a file.

### Usage

```
aifed undo <FILE> [--dry-run]
```

### Arguments

| Argument | Description      |
| -------- | ---------------- |
| `<FILE>` | Path to the file |

### Options

| Option      | Description                  |
| ----------- | ---------------------------- |
| `--dry-run` | Preview what would be undone |

### Examples

```bash
# Undo last edit
aifed undo main.rs

# Preview undo without applying
aifed undo main.rs --dry-run
```

### Undo Behavior

- Undo reverts the most recent edit operation
- Each undo moves back one entry in the history
- After undo, you can use `redo` to reapply

---

## `redo` - Redo Undone Edits

Redo the last undone edit for a file.

### Usage

```
aifed redo <FILE> [--dry-run]
```

### Arguments

| Argument | Description      |
| -------- | ---------------- |
| `<FILE>` | Path to the file |

### Options

| Option      | Description                  |
| ----------- | ---------------------------- |
| `--dry-run` | Preview what would be redone |

### Examples

```bash
# Redo last undone edit
aifed redo main.rs

# Preview redo without applying
aifed redo main.rs --dry-run
```

### Redo Behavior

- Redo reapplies the most recently undone edit
- Redo is only available after an undo
- New edits clear the redo stack

---

## Storage

History is stored in daemon memory with the following characteristics:

| Aspect      | Behavior                             |
| ----------- | ------------------------------------ |
| Location    | Daemon memory (not persisted)        |
| Scope       | Tracked independently per file       |
| Persistence | Lost on daemon restart               |
| Max entries | Configurable (default: 100 per file) |

### Implications

- **Daemon restart clears all history** - Use git commits for persistent checkpoints
- **Per-file tracking** - History is isolated between files
- **Fast operations** - In-memory storage provides quick undo/redo

---

## History vs Git

| Feature     | aifed History | Git             |
| ----------- | ------------- | --------------- |
| Granularity | Every edit    | Commits only    |
| Automatic   | Yes           | No              |
| Intent      | Edit tracking | Version control |

**Use history for:**
- Quick undo without git operations
- Debug what changed
- Short-term recovery

**Use git for:**
- Intentional checkpoints
- Collaboration
- Long-term history
- Branching

---

## Workflow Example

```bash
# 1. Make edits
aifed edit main.rs <<'EOF'
= 42:AB "new code"
EOF

# 2. Check history
aifed history main.rs

# 3. If something went wrong, undo
aifed undo main.rs

# 4. Or preview the undo first
aifed undo main.rs --dry-run

# 5. If you change your mind, redo
aifed redo main.rs
```

---

## See Also

- [Edit Commands](edit-commands.md) - Making edits
- [Read Commands](read-commands.md) - Reading files with hashlines
