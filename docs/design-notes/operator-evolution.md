# Operator Evolution

This note records the history and rationale behind the edit operators (`+`, `-`, `=`).

## Timeline

### Phase 1: Three operators (`=`, `+`, `-`)

Initially, aifed had three distinct operators:

- `=` — Replace a single line (internally stored as a `replacements` HashMap + `Operation::Replace` variant)
- `+` — Insert after a locator
- `-` — Delete at a locator (supports both single-line and range `[START:HASH,END:HASH]`)

The `=` operator maintained its own internal data structure (`replacements: HashMap<usize, String>`)
and had conflict detection (`ConflictDeleteAndReplace` error) to prevent a line from being
both deleted and replaced in the same batch.

### Phase 2: Remove `=` — "less is more"

**Commit:** `98e3e6e` (refactor(edit): remove = and support multi-line inserts)

**Reasoning:** Replace is expressible as delete + insert at the same line.
Having one fewer operator means fewer concepts. The batch system uses original
line numbers, so `- LINE:HASH` + `+ LINE:HASH` at the same line produces the
same result without a dedicated `=` operator.

**Changes made:**
- Removed `Operation::Replace` variant
- Removed `replacements` HashMap from `EditPlan`
- Removed `ConflictDeleteAndReplace` error variant
- `+` was extended to accept multiple quoted payloads (multi-line insert)
- All docs and tests updated to use `-` + `+` for replacement

### Phase 3: Restore `=` — syntactic sugar

**Commit:** `07e9a8b` (feat(edit): restore = operator as syntactic sugar for replace)

**Reasoning (the reversal):** The removal revealed that:

1. Replace is a common edit operation. Two operations (with duplicate locators)
   is more verbose and more error-prone for AI generation than one.
2. The `EditPlan` already supported delete + insert at the same line as a valid
   pattern (see `test_edit_plan_delete_and_insert_share_anchor`).
3. The old complexity (separate `replacements` HashMap, conflict detection)
   was not inherent to `=` — it was incidental to the old implementation.

**Key design decision:** `=` is implemented as syntactic sugar in
`EditPlan::add()`, not with a separate data structure:

```rust
Operation::Replace => {
    self.deletions.insert(validated.target_line);
    self.inserts
        .entry(validated.target_line)
        .or_default()
        .extend(validated.new_contents);
}
```

This means:
- No `replacements` HashMap (the old approach's main complexity)
- No `ConflictDeleteAndReplace` error (delete + insert at same line is intentional)
- `=` naturally supports multi-line replacement via `new_contents: Vec<String>`
- The `EditPlan::apply()` logic is unchanged — it already handled this pattern
- The diff output shows both delete and insert changes, which is informative

The `=` operator now supports multi-line replacement (`= 42:AB "a" "b" "c"`),
which the old implementation did not (`new_content: Option<String>` was single-line only).

## Lessons Learned

1. Removing an operator removes syntax flexibility. The internal architecture
   simplification (no HashMap, no conflict detection) didn't require removing
   `=` — it only required implementing `=` differently.

2. "One way to do it" is a useful principle but should not eliminate the most
   natural expression for common operations. The two-line `-` + `+` form is
   still available and valid; `=` is the canonical form for single-line replacement.

3. The syntactic sugar pattern (parse-time conversion to existing internal
   structures) is a good pattern for adding ergonomic syntax without
   complicating the core engine.

## Current State

| Operator | Purpose              | Notes                                        |
| -------- | -------------------- | -------------------------------------------- |
| `+`      | Insert after locator | Accepts multiple content payloads            |
| `-`      | Delete at locator    | Supports `LINE:HASH` and `[START,END]` range |
| `=`      | Replace at locator   | Syntactic sugar for `-` + `+` at same line   |

All three are parsed in `batch.rs` and processed through `EditPlan` which
internally uses only two sets: `deletions` (HashSet) and `inserts` (HashMap).
