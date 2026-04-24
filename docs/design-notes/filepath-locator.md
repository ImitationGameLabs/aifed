# Filepath and Locator Design

## Filepath and Locator Separation

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

## Column Positioning: Symbol Locator vs Numeric Column

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

## Workspace Detection: CWD Over Target File

**Decision:** Workspace detection uses the command's working directory (cwd), not the target file path.

**Rationale:**

LSP operations like `def` can navigate to third-party libraries and standard library source paths outside the project. If workspace detection searched upward from the target file path, these external files would either:

- Miss the workspace entirely (no `aifed.toml` or `.git` in global paths)
- Accidentally match an unrelated workspace marker in an ancestor directory

The cwd is always within the user's project, making it the reliable anchor point for workspace resolution.

See the [Locator Reference](../reference/locator.md) for Symbol Locator usage details.
