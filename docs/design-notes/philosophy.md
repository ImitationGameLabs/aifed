# Design Philosophy

## AI-First Principles

Every design decision prioritizes AI usage patterns over human convenience:

| Human Preference    | AI Preference        | Our Choice             |
| ------------------- | -------------------- | ---------------------- |
| Interactive prompts | Explicit arguments   | Always explicit        |
| Colored output      | Structured text/JSON | `--json` for structure |

## One Way to Do It

Avoid multiple ways to accomplish the same task. When alternatives exist, choose the clearer one and remove the other.

**Rationale:**
- Reduces decision fatigue for AI agents
- Simplifies documentation and learning
- Prevents inconsistent usage patterns

**Examples:**
- Single locator syntax (not `file:line` AND `file line`)
- Long flags only (not `-f` AND `--file`)

## Help vs Skill

aifed provides two levels of documentation:

- `--help` - Quick command reference
- `--skill` - Complete usage guide

**Design rationale:**

| Flag      | Purpose                 | Length |
| --------- | ----------------------- | ------ |
| `--help`  | Quick command discovery | Short  |
| `--skill` | Complete usage guide    | Full   |

**Implementation:**

- `--help` shows available commands and brief description
- `--skill` includes: workflow, output format, operators, locators, editing tips, examples
- When adding new features, update skill.md for agent documentation
