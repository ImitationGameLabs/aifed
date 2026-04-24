# Architecture: CLI + Daemon

## Configuration Layers

Priority (highest wins): CLI flags > Environment variables > Project config > Global config > Built-in defaults

```
1. Built-in defaults
2. Global config (~/.config/aifed/config.toml)
3. Project config (aifed.toml)
4. Environment variables (AIFED_*)
5. CLI flags (--option)
```

## History vs Git

| Feature     | aifed History | Git             |
| ----------- | ------------- | --------------- |
| Granularity | Every edit    | Commits only    |
| Automatic   | Yes           | No              |
| Intent      | Edit tracking | Version control |

**Decision:** Keep separate - different granularity, no git dependency.

## CLI + Daemon Architecture

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
