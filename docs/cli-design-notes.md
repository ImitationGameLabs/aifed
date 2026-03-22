# aifed CLI Design Notes

This document records design rationale, trade-offs, and decisions. For command usage, see the [CLI Reference](reference/README.md).

---

## Design Philosophy

### AI-First Principles

Every design decision prioritizes AI usage patterns over human convenience:

| Human Preference    | AI Preference        | Our Choice             |
| ------------------- | -------------------- | ---------------------- |
| Interactive prompts | Explicit arguments   | Always explicit        |
| Colored output      | Structured text/JSON | `--json` for structure |

### One Way to Do It

Avoid multiple ways to accomplish the same task. When alternatives exist, choose the clearer one and remove the other.

**Rationale:**
- Reduces decision fatigue for AI agents
- Simplifies documentation and learning
- Prevents inconsistent usage patterns

**Examples:**
- Single locator syntax (not `file:line` AND `file line`)
- Long flags only (not `-f` AND `--file`)

### Help vs Skill

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
  edit   Edit with operators: = (replace), + (insert), - (delete)
         Locator: LINE:HASH or 0:00 for file beginning
```

---

## Core Design Decisions

### 1. Deterministic Positioning (Hashline)

**Problem:** Line numbers drift when files change; concurrent edits cause unexpected modifications.

**Solution:** Hashline - line number + content hash combination

| Approach  | Safety            | Human-readable | Debug-friendly   |
| --------- | ----------------- | -------------- | ---------------- |
| line+hash | ✅ Dual validation | ✅ Line visible | ✅ Easy to locate |
| hash only | ✅ Content-based   | ❌ Need lookup  | ⚠️ Requires grep  |
| line only | ❌ Drift risk      | ✅ Direct       | ✅ Direct         |

See [locator.md](reference/locator.md) for usage details.

### 2. Hash Algorithm

**Decision:** xxHash64 + base32hex (2 characters, 10 bits)

| Option               | Hash Length | Entropy | Pros                             |
| -------------------- | ----------- | ------- | -------------------------------- |
| oh-my-pi (xxHash32)  | 2 chars     | 8 bit   | Very compact                     |
| **aifed (xxHash64)** | 2 chars     | 10 bit  | Compact, 4x lower collision rate |

**Our approach:**

- Use xxHash64 to hash the raw line content (whitespace preserved)
- Take the top 10 bits of the hash
- Encode as 2 characters using base32hex (`0-9`, `A-V`)

**Comparison with oh-my-pi:**

| Aspect            | oh-my-pi                   | aifed             |
| ----------------- | -------------------------- | ----------------- |
| Hash algorithm    | xxHash32                   | xxHash64          |
| Character set     | 16-char custom (`ZPMQ...`) | 32-char base32hex |
| Encoded bits      | 8 bit                      | 10 bit            |
| Collision rate    | 1/256                      | 1/1024            |
| Whitespace        | Stripped                   | Preserved         |
| Symbol-only lines | Mix in line number         | No special case   |

**Design rationale:**
- **base32hex**: Standard RFC 2938 encoding, good readability. Using 32 characters instead of 16 keeps the same 2-character length while reducing collision rate 4x (from 1/256 to 1/1024) at essentially no cost
- **Preserve whitespace**: Whitespace is part of content; `foo` and `  foo` should have different hashes
- **No symbol-only special case**: Simplifies implementation, relies on 10-bit low collision rate
- **Independent line hashes**: Each line's hash is computed independently, not chained to neighbors

**Why independent hashes over chaining:**

| Aspect                    | Independent Hashes        | Chaining Hashes                                         |
| ------------------------- | ------------------------- | ------------------------------------------------------- |
| Hash stability            | Same content -> same hash | Same content -> different hash (if lines above changed) |
| Concurrent edit detection | Target line only          | Target line and all lines above                         |

### 3. Command Structure: Unified Edit

**Decision:** Use a single `edit` command with operator prefixes (`=`/`+`/`-`) instead of separate `replace`/`insert`/`delete` commands.

```
= <LOCATOR> <CONTENT>   # replace
+ <LOCATOR> <CONTENT>   # insert after
- <LOCATOR>             # delete
```

**Rationale:**

| Aspect           | Unified `edit`        | Split verbs (replace/insert/delete) |
| ---------------- | --------------------- | ----------------------------------- |
| Decision fatigue | Single entry point    | Must choose which command           |
| Learning curve   | Three operators       | Four commands                       |
| Consistency      | One syntax for all    | Different per-command syntax        |
| Token efficiency | Operators are compact | Command names are longer            |
| Help text        | One place to look     | Multiple man pages                  |

**Why operators over subcommands:**

Using symbolic operators (`=`/`+`/`-`) instead of subcommands (`replace`/`insert`/`delete`):
- **Consistency:** Same syntax for single and batch operations
- **Token efficiency:** Shorter, especially in batch mode
- **Visual clarity:** Operators visually distinguish operation types
- **No decision point:** Single command entry eliminates "which command?" decision

**Operator choice for replace:**

| Operator | Pros                             | Cons                        |
| -------- | -------------------------------- | --------------------------- |
| `~`      | Common in editors (vim `~` case) | Shell expands to `$HOME`    |
| `=`      | Shell-safe, assignment semantics | Less common in text editors |

**Decision:** Use `=` for replace operation.

**Rationale:** `~` is expanded by shells (bash, zsh, etc.) to the user's home directory. This requires quoting (`'~'`) on every invocation, which is error-prone and adds friction. `=` is shell-safe and conveys "assignment" semantics (replace X with Y).

**Virtual Line Convention:**

`0:00` represents the position before the first line, enabling insert-at-beginning:

```bash
aifed edit main.rs + 0:00 "// Copyright 2026"
```

### 4. Filepath and Locator Separation

**Question:** Should filepath and locator be combined (`FILE:LOCATOR`) or separated (`FILE LOCATOR`)?

**Decision:** Use positional arguments to separate filepath from locator.

**Comparison:**

| Approach       | Syntax              | Token Cost | AI Parsing   | Implementation |
| -------------- | ------------------- | ---------- | ------------ | -------------- |
| Colon-joined   | `main.rs:15`        | 2-3        | Split `:`    | Windows issues |
| Flags          | `main.rs --line 15` | 4-5        | Simple       | Low            |
| **Positional** | `main.rs 15`        | **2-3**    | **Simplest** | **Simplest**   |

**Why positional arguments:**

1. **Token efficient** - Same cost as colon-joined, better than flags
2. **AI-friendly** - Follows "command + arg1 + arg2" pattern naturally
3. **Implementation simplicity** - No string splitting, no Windows path conflicts
4. **Better error messages** - FILE and LOCATOR validated independently

**Why not colon-joined:**

- Requires parsing to separate file path from locator
- Windows paths (`C:\path:15`) create ambiguous `:` characters
- Industry convention (vim/grep) is less relevant for AI users

### 5. Column Positioning: Symbol Locator vs Numeric Column

**Problem:** LLMs cannot reliably count character positions. Numeric columns are error-prone.

**Decision:** Replace column numbers with Symbol Locators.

**Design:**

| Locator Type   | Format        | Use Case                   | Token Cost |
| -------------- | ------------- | -------------------------- | ---------- |
| Line Locator   | `LINE:HASH`   | Edit operations (default)  | Low        |
| Symbol Locator | `SINDEX:NAME` | LSP operations (on-demand) | Higher     |

**Why Symbol Locator:**

1. **No counting** - Index is sequential, not character offset
2. **Verifiable** - LSP validates symbol existence
3. **Self-documenting** - NAME makes locator readable
4. **On-demand** - Only output when `symbols` command is used with a line
5. **Token efficient** - Normal edits don't pay the symbol overhead

**Why no hash in Symbol Locator:**

Unlike Line Locator, Symbol Locator doesn't include a hash because:
- Symbol names are typically short, so hash provides minimal compression benefit
- LSP already validates symbol existence
- Symbol name provides semantic verification

**Comparison with alternatives:**

| Approach       | Format    | AI-friendly | Verifiable | Extra Read |
| -------------- | --------- | ----------- | ---------- | ---------- |
| Numeric column | `15:10`   | No          | No         | No         |
| Symbol Locator | `S1:user` | Yes         | Yes (LSP)  | Yes        |
| Text fragment  | `"user"`  | Partial     | Partial    | No         |

**Text fragment alternative (rejected):**

```bash
aifed hover main.rs:15:"let user"
```

Pros:
- No extra `symbols` step
- Works with already-read content

Cons:
- Ambiguity: same text may appear multiple times
- Context needed for uniqueness, increasing length
- Less precise than semantic symbol matching

**Decision:** Use Symbol Locator despite extra read requirement - precision and reliability outweigh the cost.

See [locator.md](reference/locator.md) for Symbol Locator usage details.

---

## Key Trade-offs

### Hash Mismatch Behavior

When hash doesn't match current line content:

| Option                  | Pros           | Cons                |
| ----------------------- | -------------- | ------------------- |
| Reject + show current   | Safe, clear    | Requires re-read    |
| Reject + show diff      | More context   | More complex output |
| Prompt for confirmation | Human-friendly | Not AI-friendly     |

**Choice:** Reject with error + show current line/hash. This forces AI to re-read the file, ensuring it has the correct current state before editing.

### Batch Operations: Line Number Drift

**Problem:** Earlier edits shift line numbers for later operations.

**Solution:** Use hashes instead of line numbers in batch - they're content-based and remain valid.

### Batch Failure Handling

| Option                  | Pros             | Cons                    |
| ----------------------- | ---------------- | ----------------------- |
| All-or-nothing (atomic) | No partial state | All lost on one failure |
| Best-effort             | Some progress    | Complex partial state   |
| Stop on first failure   | Simple           | No feedback on rest     |

**Choice:** Atomic (all-or-nothing). If any operation fails, none are applied.

### History vs Git

| Feature     | aifed History | Git             |
| ----------- | ------------- | --------------- |
| Granularity | Every edit    | Commits only    |
| Automatic   | Yes           | No              |
| Intent      | Edit tracking | Version control |

**Decision:** Keep separate - different granularity, no git dependency.

---

## Configuration Layers

Priority (highest wins): CLI flags > Environment variables > Project config > Global config > Built-in defaults

```
1. Built-in defaults
2. Global config (~/.config/aifed/config.toml)
3. Project config (aifed.toml)
4. Environment variables (AIFED_*)
5. CLI flags (--option)
```

---

## Error Handling

### Error Format

```
Error: Hash mismatch
  File: main.rs
  Expected hash: AB
  Actual hash: 3K
  Actual content: fn main() {
  Hint: Run 'aifed read main.rs' to get current hashes
```

### No Exit Codes

**Decision:** aifed does not use semantic exit codes.

**Rationale:**

aifed is designed exclusively for AI agents, not for shell integration. Traditional exit codes (e.g., 3 for file not found, 4 for hash mismatch) serve shell scripts and CI pipelines where programs need to branch based on numeric codes.

For AI agents:
- They parse error messages or JSON output directly
- Numeric codes add no value
- Unit tests match `Result<T, Error>` types, not exit codes
- E2E tests can verify error message content

Shell users have better alternatives: `sed`, `awk`, `ed` for scripting needs. aifed's value proposition is AI-first editing, not general-purpose shell integration.

**Implementation:** Simple binary exit: 0 for success, 1 for any error.

### 4. Architecture: CLI + Daemon

**Decision:** aifed uses CLI + daemon architecture.

**Rationale:**

LSP servers have significant startup cost. For example, rust-analyzer can take seconds to initialize and index a project. A daemonless approach would require starting LSP server on every CLI invocation, wait for initialization, execute a single operation, then shutdown. This is impractical for interactive use.

Daemon architecture keeps LSP servers running in background, providing instant responses.

**Comparison (for reference):**

| Aspect          | CLI + Daemon                              | Daemonless CLI         |
| --------------- | ----------------------------------------- | ---------------------- |
| **Startup**     | Daemon startup overhead, connection setup | Instant                |
| **History**     | In-memory, fast access, centralized       | File-based, slower     |
| **LSP**         | Background tasks, pre-warmed servers      | Per-invocation startup |
| **State**       | Shared state across invocations           | No persistent state    |
| **Deployment**  | Daemon lifecycle management               | Simple, single binary  |
| **Portability** | Requires daemon running                   | Self-contained         |

**Daemon responsibilities:**
- Maintain persistent LSP server connections
- Background indexing and diagnostics
- In-memory history with fast undo/redo
- Shared file cache across CLI invocations
- Watch mode and reactive features

**Workspace management:**
- Single daemon manages multiple project workspaces
- Project identified by root directory (git root or `aifed.toml` location)
- Detailed design TBD

**Lightweight mode consideration:**

For simple file edits without LSP or workspace management, a lightweight mode should be considered:

- **Use case:** Quick edits on standalone files (e.g., config files, notes, scripts)
- **Options:**
  - `--no-daemon` flag to skip daemon connection
  - Auto-detect: skip daemon if no LSP commands used
  - Separate lightweight commands (e.g., `aifed edit-quick`)
- **Trade-off:** Simplicity vs. consistency of CLI interface

Detailed design TBD.

---

## Open Questions

1. **Concurrent editing** - Current: hash-based optimistic locking. Future: consider merge strategies.
2. **Binary files** - Current: reject with clear error. Focus on text editing.
3. **Remote files** - Current: no. Use sshfs or similar.
4. **Plugin system** - Defer. Hooks provide some extensibility.

---

## Future Considerations

### v2 Candidates
- Deeper git integration
- Plugin system
