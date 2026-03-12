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
aifed snapshot create main.go --tag before-refactor

# List snapshots
aifed snapshot list main.go

# Restore to previous state
aifed snapshot restore main.go --tag before-refactor

# Delete old snapshot
aifed snapshot delete main.go --tag old-snapshot
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
aifed history main.go

# Show last 20 edits
aifed history main.go --last 20

# Show full diffs
aifed history main.go --full

# JSON output
aifed history main.go --json
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

### Storage Implementation Options

The history feature can be implemented using different storage backends:

| Option          | Pros                                       | Cons                                             |
| --------------- | ------------------------------------------ | ------------------------------------------------ |
| **Daemon**      | Fast in-memory access, centralized state   | Requires background process, state loss on crash |
| **File-system** | Simple, no dependencies, portable          | Slower for large histories, file I/O overhead    |
| **SQLite**      | Efficient queries, indexing, atomic writes | External dependency, database management         |

**Daemon-based:**
- CLI communicates with a background daemon process
- Daemon maintains history in memory, periodically persists to disk
- Fast operations, but requires daemon lifecycle management

**File-system:**
- Direct file storage in `.aifed/history/`
- Each edit stored as a separate file or appended to a log
- Simple and portable, no background processes

**SQLite:**
- Single database file `.aifed/history.db`
- Efficient querying with indexes
- Built-in support for transactions and atomic operations

**Decision:** TBD - Will evaluate based on performance requirements and deployment complexity.

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
aifed undo main.go

# Undo last 3 edits
aifed undo main.go --steps 3

# Preview undo
aifed undo main.go --dry-run
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
aifed snapshot create main.go --tag before-refactor

# 2. Make edits
aifed replace main.go:42:abc123 "new code"

# 3. Check history
aifed history main.go

# 4. If something went wrong, undo
aifed undo main.go

# 5. Or restore to snapshot
aifed snapshot restore main.go --tag before-refactor

# 6. Clean up snapshot when done
aifed snapshot delete main.go --tag before-refactor
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
