# aifed - AI-First Editor (Early Stage)

A text editor designed for AI agents.

## Core Philosophy

aifed's goal is to design a dedicated text editor for AI agents, implemented as a CLI, improving AI's editing efficiency, accuracy, and comfort.

- **CLI is the best UI for AI** - Text input/output, structured responses
- **Progressive disclosure** - Simple commands for simple tasks, advanced features on demand
- **Decoupled from AI Agents** - Any coding agent that supports shell can use it

## Getting Started

See the [Installation Guide](docs/installation-guide.md) for setup instructions.

## Current Status

aifed is in early development. Currently implemented features:

| Feature                                       | Status      |
| --------------------------------------------- | ----------- |
| `read` - Read file content with hashlines     | Implemented |
| `edit` - Edit file with hashline verification | Implemented |
| `lsp` - LSP integration (Rust first)          | Implemented |
| `history/undo` - Edit history and recovery    | Planned     |

## Usage Examples

> **Note:** The examples below illustrate the envisioned workflow. Currently, `read`, `edit`, and `lsp` commands are implemented.

AI agents can obtain the full usage guide with:
```bash
aifed --skill
```

Here's how an AI agent might work with aifed in typical development scenarios:

### Understanding Code

```bash
# Read line 15 of main.rs with hashlines for safe editing
aifed read main.rs 15
```
```
15:3K|let count = calculate_total(items);
```

```bash
# Get symbol locators on line 15 for LSP operations
aifed lsp symbols main.rs 15
```
```
15:3K|let count = calculate_total(items);
S1:count
S2:calculate_total
S3:items
```

```bash
# Get type info for calculate_total function
aifed lsp hover main.rs 15:3K S2:calculate_total

# Go to definition of items
aifed lsp def main.rs 15:3K S3:items
```

### Refactoring

```bash
# Get symbol locators on line 10
aifed lsp symbols main.rs 10
```
```
10:AB|let config = load_config();
S1:config
S2:load_config
```

```bash
# Find all references to config
aifed lsp refs main.rs 10:AB S1:config

# Rename config to settings across the codebase
aifed lsp rename main.rs 10:AB S1:config settings

# Edit line 10 with hashline verification
aifed edit main.rs = 10:AB "let settings = load_config();"
```

### Debugging

```bash
# Check for errors after changes
aifed lsp diag main.rs
```

### History & Recovery

```bash
# View recent edit history
aifed history main.rs --last 5

# Undo if something went wrong
aifed undo main.rs
```

## Documentation

- [CLI Reference](docs/reference/README.md) - Command documentation
- [CLI Design Notes](docs/cli-design-notes.md) - Design rationale and trade-offs

## Credits

This project was inspired by:

- [agent-browser](https://github.com/vercel-labs/agent-browser) - A browser designed for AI agents
- [oh-my-pi](https://github.com/can1357/oh-my-pi) - The [Hashline Edit Mode](https://deepwiki.com/can1357/oh-my-pi/8.1-hashline-edit-mode) inspired our [Locator](docs/reference/locator.md) design

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for ways to contribute to the project.
