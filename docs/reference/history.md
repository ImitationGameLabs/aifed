# History & Snapshots

Commands for managing file history and snapshots.

## Overview

aifed provides two mechanisms for tracking and reverting changes:

- **History** - Automatic tracking of every edit
- **Snapshots** - Explicit save points for recovery

---

## `snapshot` - Manage File Snapshots

Create, list, or restore file snapshots for safe editing.

### Usage

```
aifed snapshot create <FILE> [--tag <TAG>]
aifed snapshot list <FILE>
aifed snapshot restore <FILE> [--tag <TAG>]
aifed snapshot delete <FILE> --tag <TAG>
```

### Subcommands

| Subcommand | Description               |
| ---------- | ------------------------- |
| `create`   | Create a new snapshot     |
| `list`     | List snapshots for a file |
| `restore`  | Restore to a snapshot     |
| `delete`   | Delete a snapshot         |

### Options

| Option        | Description               |
| ------------- | ------------------------- |
| `--tag <TAG>` | Tag name for the snapshot |

### Storage

Snapshots are stored in `.aifed/snapshots/` within the project directory.

### Examples

```bash
# Create snapshot before risky edit
aifed snapshot create main.rs --tag before-refactor

# List snapshots
aifed snapshot list main.rs

# Restore to previous state
aifed snapshot restore main.rs --tag before-refactor

# Delete old snapshot
aifed snapshot delete main.rs --tag old-snapshot
```

### Snapshot Retention

Default configuration:
- `max_snapshots = 10` per file
- `max_age = 7 days`

Configure in `.aifed.toml`:

```toml
[snapshot]
dir = ".aifed/snapshots"
max_snapshots = 10
max_age = "7d"
```

### Snapshot Content

Snapshots store full file content (not diffs) for:
- Simple restore operation
- No dependency chain issues
- Fast recovery

---

## `history` - View Edit History

View edit history for a file.

### Usage

```
aifed history <FILE>
```

### Options

| Option       | Description                     |
| ------------ | ------------------------------- |
| `--last <N>` | Show last N edits [default: 10] |
| `--full`     | Show full diff for each edit    |

### Examples

```bash
# View recent history
aifed history main.rs

# Show last 20 edits
aifed history main.rs --last 20

# Show full diffs
aifed history main.rs --full

# JSON output
aifed history main.rs --json
```

### History vs Git

| Feature     | aifed History | Git             |
| ----------- | ------------- | --------------- |
| Granularity | Every edit    | Commits only    |
| Automatic   | Yes           | No              |
| Intent      | Edit tracking | Version control |

**Use history for:**
- Quick undo without git operations
- Debug what changed
- Audit trail

**Use git for:**
- Intentional checkpoints
- Collaboration
- Branching

### Storage Implementation

aifed uses a daemon architecture (required for LSP performance), but history storage backend is configurable:

| Option          | Pros                                       | Cons                                          | Use Case           |
| --------------- | ------------------------------------------ | --------------------------------------------- | ------------------ |
| **Memory**      | Fastest access, simple                     | State loss on crash                           | Default            |
| **File-system** | Persistent, no dependencies, portable      | Slower for large histories, file I/O overhead | Simple persistence |
| **SQLite**      | Efficient queries, indexing, atomic writes | External dependency, database management      | Large histories    |

**Memory (default):**
- Daemon maintains history in memory
- Fastest operations
- State lost on daemon restart/crash
- Use snapshots for intentional save points

**File-system:**
- Direct file storage in `.aifed/history/`
- Each edit stored as a separate file or appended to a log
- Persistent across daemon restarts
- No external dependencies

**SQLite:**
- Single database file `.aifed/history.db`
- Efficient querying with indexes
- Built-in support for transactions and atomic operations
- Best for large history volumes

**Configuration:**

```toml
[history]
backend = "memory"  # Options: memory, filesystem, sqlite
max_entries = 100
```

---

## `undo` - Undo Recent Edits

Undo recent edits.

### Usage

```
aifed undo <FILE>
```

### Options

| Option        | Description                          |
| ------------- | ------------------------------------ |
| `--steps <N>` | Number of edits to undo [default: 1] |
| `--dry-run`   | Preview what would be undone         |

### Examples

```bash
# Undo last edit
aifed undo main.rs

# Undo last 3 edits
aifed undo main.rs --steps 3

# Preview undo
aifed undo main.rs --dry-run
```

### Undo Granularity

| Method             | Scope             |
| ------------------ | ----------------- |
| `undo`             | Single edit       |
| `undo --steps N`   | Multiple edits    |
| `snapshot restore` | To specific point |

## Workflow Example

```bash
# 1. Create snapshot before major changes
aifed snapshot create main.rs --tag before-refactor

# 2. Make edits
aifed replace main.rs 42:abc123 "new code"

# 3. Check history
aifed history main.rs

# 4. If something went wrong, undo
aifed undo main.rs

# 5. Or restore to snapshot
aifed snapshot restore main.rs --tag before-refactor

# 6. Clean up snapshot when done
aifed snapshot delete main.rs --tag before-refactor
```

## Configuration

```toml
[history]
enabled = true
max_entries = 100

[snapshot]
dir = ".aifed/snapshots"
max_snapshots = 10
max_age = "7d"
```

## See Also

- [Edit Commands](edit-commands.md) - Making edits
- [Configuration](configuration.md) - History and snapshot settings
- [Utilities](utilities.md) - Using diff to compare versions
