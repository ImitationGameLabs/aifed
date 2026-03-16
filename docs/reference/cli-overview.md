# CLI Overview

Global options and environment variables for aifed.

## Usage

```
aifed [OPTIONS] <COMMAND>
```

## Global Options

| Option            | Description                                    |
| ----------------- | ---------------------------------------------- |
| `--json`          | Output in JSON format                          |
| `--no-color`      | Disable colored output                         |
| `--quiet`         | Suppress non-essential output                  |
| `--dir <DIR>`     | Change working directory before executing      |
| `--config <FILE>` | Configuration file path [default: .aifed.toml] |
| `--help`          | Print help                                     |
| `--version`       | Print version                                  |

## Environment Variables

| Variable         | Description                            |
| ---------------- | -------------------------------------- |
| `AIFED_CONFIG`   | Path to configuration file             |
| `AIFED_NO_COLOR` | Disable colored output (`1` or `true`) |
| `AIFED_JSON`     | Default to JSON output (`1` or `true`) |
| `AIFED_QUIET`    | Quiet mode (`1` or `true`)             |

## AI-First Design Principles

aifed is designed specifically for AI agents, prioritizing:

1. **Clarity over brevity** - Long flags only (`--file` not `-f`)
2. **Explicit arguments** - No interactive prompts
3. **Structured output** - `--json` for machine-parseable responses
4. **Deterministic positioning** - Hashline for safe, verifiable edits

## See Also

- [Command Reference](README.md) - All commands
- [Configuration](configuration.md) - Configuration file format
- [CLI Design Notes](../cli-design-notes.md) - Design rationale
