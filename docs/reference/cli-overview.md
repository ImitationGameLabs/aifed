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

## AI-First Design Principles

aifed is designed specifically for AI agents, prioritizing:

1. **Clarity over brevity** - Long flags only (`--file` not `-f`)
2. **Explicit arguments** - No interactive prompts
3. **Structured output** - `--json` for machine-parseable responses
4. **Deterministic positioning** - Hashline for safe, verifiable edits

## See Also

- [Command Reference](README.md) - All commands
- [CLI Design Notes](../cli-design-notes.md) - Design rationale
