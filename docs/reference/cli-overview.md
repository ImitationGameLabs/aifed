# CLI Overview

Global options for aifed.

## Usage

```
aifed [OPTIONS] <COMMAND>
```

## Global Options

| Option      | Description                |
| ----------- | -------------------------- |
| `--json`    | Output in JSON format      |
| `--skill`   | Print complete usage guide |
| `--help`    | Print help                 |
| `--version` | Print version              |

## Workspace Detection

aifed automatically detects the workspace root by searching upward from the current directory.

At each directory level, it checks for both markers:

1. `aifed.toml` - Project configuration file (marks project root)
2. `.git` - Git repository root

The **closest** marker wins. At the same level, `aifed.toml` takes priority over `.git`.

This ensures nested git repositories are not affected by external `aifed.toml` files.

If no workspace is detected, only `read` and `edit` commands are available (lightweight mode).

## Daemon Auto-Start Behavior

The CLI automatically manages the daemon lifecycle based on the command being executed.

### Daemon Requirements by Command

| Command         | Auto-Start | Heartbeat | When Unavailable                                    |
| --------------- | :--------: | :-------: | --------------------------------------------------- |
| `--help`        |     No     |    No     | N/A                                                 |
| `--skill`       |     No     |    No     | N/A                                                 |
| `info`          |     No     |    No     | N/A                                                 |
| `daemon status` |     No     |    No     | Reports "Daemon not running" + workspace + log path |
| `daemon stop`   |     No     |    No     | Reports "Daemon not running"                        |
| `read`          |    Yes     |    Yes    | Warning + degraded mode                             |
| `edit`          |    Yes     |    Yes    | Warning + degraded mode                             |
| `lsp *`         |    Yes     |    Yes    | Error, cannot execute                               |
| `history`       |    Yes     |    Yes    | Error, cannot execute                               |
| `undo`          |    Yes     |    Yes    | Error, cannot execute                               |
| `redo`          |    Yes     |    Yes    | Error, cannot execute                               |

### Degraded Mode Warning

When `read` or `edit` commands cannot connect to the daemon, they operate in degraded mode:

```
Warning: daemon unavailable. The following features are disabled:
  - Edit history tracking (undo/redo)
  - Concurrent modification detection
File operations will proceed without these protections.
```

## AI-First Design Principles

aifed is designed specifically for AI agents, prioritizing:

1. **Clarity over brevity** - Long flags only (`--file` not `-f`)
2. **Explicit arguments** - No interactive prompts
3. **Structured output** - `--json` for machine-parseable responses
4. **Deterministic positioning** - Hashline for safe, verifiable edits

## See Also

- [Command Reference](README.md) - All commands
- [Design Notes](../design-notes/README.md) - Design rationale
