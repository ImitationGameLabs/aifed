# Edit Model

## Command Structure: Unified Edit

**Decision:** Use a single `edit` command with operator prefixes (`+`/`-`) instead of separate `replace`/`insert`/`delete` commands.

```
+ <LOCATOR> <CONTENT...>   # insert one or more lines after locator
- <LOCATOR>                # delete
= <LOCATOR> <CONTENT>      # replace line content (delete + insert in one step)
```

**Rationale:**

| Aspect           | Unified `edit`        | Split verbs (replace/insert/delete) |
| ---------------- | --------------------- | ----------------------------------- |
| Decision fatigue | Single entry point    | Must choose which command           |
| Learning curve   | Two operators         | Four commands                       |
| Consistency      | One syntax for all    | Different per-command syntax        |
| Token efficiency | Operators are compact | Command names are longer            |
| Help text        | One place to look     | Multiple man pages                  |

**Why operators over subcommands:**

Using symbolic operators (`+`/`-`/`=`) instead of subcommands (`replace`/`insert`/`delete`):
- **Consistency:** Same syntax for single and batch operations
- **Token efficiency:** Shorter, especially in batch mode
- **Visual clarity:** Operators visually distinguish operation types
- **No decision point:** Single command entry eliminates "which command?" decision

## Replacement Model

The `=` operator provides first-class replacement support. It is syntactic sugar
for delete plus insert at the same locator:

```text
= 42:AB "new content"
```

This is equivalent to:

```text
- 42:AB
+ 42:AB "new content"
```

**Engine behavior:** Internally, `=` expands to a deletion + insertion at the
same anchor line. Because all operations are anchored to the original file state,
no index-shift issues arise.

For range replacement, the documented canonical form is:

```text
- [START:HASH,END:HASH]
+ END:HASH "new line 1" "new line 2"
```

- **Engine behavior:** because all operations are anchored to the original file state, inserting after the line before the deleted range or any deleted line may still deterministically land at the same replacement point.
- **Documented canonical form:** only teach `+ END:HASH ...` so LLMs can reuse a locator already present in the delete range, avoid `start-1` arithmetic, and avoid special-casing ranges that start at line 1.

## Single-line Delete

Use the bare hashline form for single-line delete:

```text
- 42:AB     # preferred
```

Avoid the range form for single-line deletes:

```text
- [42:AB,42:AB]   # valid but unnecessarily verbose
```

The range form `[START:HASH,END:HASH]` is intended for multi-line range deletion only.

## Virtual Line Convention

`0:00` represents the position before the first line, enabling insert-at-beginning:

```bash
aifed edit main.rs + 0:00 "// Copyright 2026"
```

## String Content

Content in double quotes supports JSON escape sequences (`\"`, `\\`, `\t`, `\r`, `\uXXXX`).
`\n` is rejected — each content payload is exactly one line. Use multiple quoted payloads
to insert multiple lines at one locator.
