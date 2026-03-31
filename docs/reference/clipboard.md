# Clipboard

> **Status: Implemented** - Copy, Paste, Clipboard preview

Commands for copying lines between files using a workspace-scoped clipboard.

## Overview

aifed provides a clipboard that stores text content in the daemon. Copy lines from one file, then paste them into another. The clipboard is workspace-scoped and in-memory only.

---

## `copy` - Copy Lines to Clipboard

Copy lines from a file to the daemon clipboard.

### Usage

```
aifed copy <FILE> <RANGE>
```

### Arguments

| Argument  | Description                            |
| --------- | -------------------------------------- |
| `<FILE>`  | Path to the file                       |
| `<RANGE>` | Hashline range (e.g., `"[1:AB,5:CD]"`) |

### Examples

```bash
# Copy lines 10-20
aifed copy main.rs "[10:AB,20:CD]"
```

### Output

```
Copied 3 line(s) (2-4) from main.rs
2|    let x = 1;
3|    let y = 2;
4|    let z = x + y;
```

---

## `paste` - Paste Clipboard Content to File

Paste the current clipboard content into a file at a specified position.

### Usage

```
aifed paste <FILE> <LINE:HASH>
```

### Arguments

| Argument      | Description                                         |
| ------------- | --------------------------------------------------- |
| `<FILE>`      | Path to the file                                    |
| `<LINE:HASH>` | Hashline position to insert after (e.g., `"10:AB"`) |

### Examples

```bash
# Paste after line 20
aifed paste main.rs 20:CD

# Paste at file beginning
aifed paste main.rs 0:00
```

### Output

```
Applied 3 operations to main.rs, 3 insertions(+)
 18|    let a = 1;
+19|    let x = 1;
+20|    let y = 2;
+21|    let z = x + y;
 22|    let b = 2;
```

---

## `clipboard` - Show Clipboard Content

Display the current clipboard content.

### Usage

```
aifed clipboard
```

### Examples

```bash
# Show clipboard
aifed clipboard
```

### Output

```
    let x = 1;
    let y = 2;
    let z = x + y;
```

If the clipboard is empty:

```
Clipboard is empty
```

---

## Storage

| Aspect      | Behavior                      |
| ----------- | ----------------------------- |
| Location    | Daemon memory (not persisted) |
| Scope       | Workspace-scoped              |
| Capacity    | Single entry (overwrites)     |
| Persistence | Lost on daemon restart        |

### Implications

- **Daemon restart clears the clipboard** - Copy/paste is for short-term use within a session
- **Workspace isolation** - Each workspace has its own clipboard
- **Single entry** - New copy overwrites the previous content

---

## Workflow Example

```bash
# 1. Read source file and copy lines
aifed read utils.rs
aifed copy utils.rs "[5:AB,12:CD]"

# 2. Read target file and paste
aifed read main.rs
aifed paste main.rs 15:3K

# 3. Verify clipboard content
aifed clipboard
```

---

## See Also

- [Edit Commands](edit-commands.md) - Making edits with hashlines
- [Read Commands](read-commands.md) - Reading files with hashlines
- [History](history.md) - Undo/redo for edit recovery
