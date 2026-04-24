# Batch Operations & Atomicity

## Hash Mismatch Behavior

When hash doesn't match current line content:

| Option                  | Pros           | Cons                |
| ----------------------- | -------------- | ------------------- |
| Reject + show current   | Safe, clear    | Requires re-read    |
| Reject + show diff      | More context   | More complex output |
| Prompt for confirmation | Human-friendly | Not AI-friendly     |

**Choice:** Reject with error + show current line/hash. This forces AI to re-read the file, ensuring it has the correct current state before editing.

## Line Number Drift

**Problem:** Earlier edits shift line numbers for later operations.

**Solution:** Batch operations reference the original file state (1-based line numbers from the file as-read). The `EditPlan` resolves all edits against this fixed state, then applies them atomically. Hashes are content-based, so they remain valid regardless of other edits in the batch.

## Failure Handling

| Option                  | Pros             | Cons                    |
| ----------------------- | ---------------- | ----------------------- |
| All-or-nothing (atomic) | No partial state | All lost on one failure |
| Best-effort             | Some progress    | Complex partial state   |
| Stop on first failure   | Simple           | No feedback on rest     |

**Choice:** Atomic (all-or-nothing). If any operation fails, none are applied.

## Single-line Range Form

For single-line delete, prefer `- LINE:HASH` over `- [LINE:HASH,LINE:HASH]`:

```text
- 42:AB     # preferred
- [42:AB,42:AB]   # valid but unnecessarily verbose
```

The range form `[START:HASH,END:HASH]` is intended for multi-line range deletion.
