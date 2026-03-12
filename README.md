# aifed - AI-First Editor

A text editor designed for AI agents.

> **Early Stage Project** - Currently in the design and documentation phase. No implementation yet.

## Core Philosophy

aifed's goal is to design a dedicated text editor for AI agents, implemented as a CLI, improving AI's editing efficiency, accuracy, and comfort.

## Inspiration

This project was inspired by [agent-browser](https://github.com/vercel-labs/agent-browser) - a browser designed for AI agents.

- **CLI is the best UI for AI** - Text input/output, structured responses
- **Progressive disclosure** - Simple commands for simple tasks, advanced features on demand
- **Decoupled from AI Agents** - Any coding agent that supports shell can use it

## Usage Examples

Here's how an AI agent might work with aifed in typical development scenarios:

### Understanding Code

```bash
# Read a specific function with hashes for later editing
aifed read main.go:42-80

# Check a symbol's type and signature
aifed hover main.go:55:10

# Navigate to a symbol's definition
aifed definition main.go:55:10
```

### Refactoring

```bash
# Confirm the scope of a symbol's usage
aifed references main.go --symbol oldName

# Rename across the entire codebase
aifed rename main.go:15:6 newName

# Get current hashes, then safely replace code
aifed read main.go:20-30
aifed replace main.go:25:abc12345 "refactored code"
```

### Debugging

```bash
# Check for errors after changes
aifed diagnostics main.go
```

### History & Recovery

```bash
# View recent edit history
aifed history main.go --last 5

# Undo if something went wrong
aifed undo main.go
```

## Documentation

- [CLI Reference](docs/reference/README.md) - Command documentation
- [CLI Design Notes](docs/cli-design-notes.md) - Design rationale and trade-offs

## Development

This project uses **Nix** + **direnv** + **crane** + **rust-overlay**.

```bash
# Auto-load environment with direnv (recommended)
direnv allow

# Or manually enter dev shell
nix develop
```
