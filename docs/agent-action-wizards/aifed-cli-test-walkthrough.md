# aifed CLI Test Walkthrough

This document is a **test suite walkthrough** for AI agents to verify the aifed CLI functionality. Each test case has clear goals, steps, and expected results.

## Test Environment

All tests run in the `.playground` directory. The aifed binary is built in debug mode.

```bash
cd .playground
```

**Note:** Hash values in expected outputs are illustrative. Actual hashes depend on content.

---

## Read File with Hashlines

**Goal:** Verify `aifed read` outputs file content with correct hashline format (LINE:HASH).

**Precondition:** A test file exists with known content.

**Steps:**
```bash
echo -e "line1\nline2\nline3" > test.txt
aifed read test.txt
```

**Expected:**
- Output format: `LINE:HASH|CONTENT`
- Each line has a 2-character base32hex hash
- Hashes are deterministic (same content = same hash)

```
1:AB|line1
2:3K|line2
3:7M|line3
```

---

## Read File Without Hashes

**Goal:** Verify `--no-hashes` flag outputs plain content without hash prefixes.

**Steps:**
```bash
aifed read test.txt --no-hashes
```

**Expected:**
- Output is plain file content without any prefixes

```
line1
line2
line3
```

---

## Read Specific Line

**Goal:** Verify line-only locator reads a single line.

**Steps:**
```bash
aifed read test.txt 2
```

**Expected:**
- Output contains only line 2 with its hashline

```
2:3K|line2
```

---

## Read Line Range

**Goal:** Verify range locator reads multiple consecutive lines.

**Steps:**
```bash
aifed read test.txt [1,2]
```

**Expected:**
- Output contains lines 1-2 with hashlines

```
1:AB|line1
2:3K|line2
```

---

## Replace Line with `=` Operator

**Goal:** Verify `=` operator replaces a line in one step.

**Steps:**
```bash
aifed read test.txt
# Capture hash for line 2 (e.g., 3K)
aifed edit test.txt <<'EOF'
= 2:3K "modified line2 with ="
EOF
aifed read test.txt
```

**Expected:**
- Line 2 content is changed
- Same result as `-` + `+` but in a single operation

```
1:AB|line1
2:P2|modified line2 with =
3:7M|line3
```

---

## Replace Line with Delete Plus Insert

**Goal:** Verify replacement via `-` plus `+` modifies a line when hash matches.

**Steps:**
```bash
aifed read test.txt
# Capture hash for line 2 (e.g., 3K)
aifed edit test.txt <<'EOF'
- 2:3K
+ 2:3K "modified line2"
EOF
aifed read test.txt
```

**Expected:**
- Line 2 content is changed
- Hash for line 2 is different (new content = new hash)

```
1:AB|line1
2:P2|modified line2
3:7M|line3
```

---

## Replacement with Hash Mismatch

**Goal:** Verify replacement is rejected when the stale hash does not match current content.

**Steps:**
```bash
# Using stale hash (from original line content)
aifed edit test.txt <<'EOF'
- 2:3K
+ 2:3K "should fail"
EOF
```

**Expected:**
- Command fails
- File is not modified
- Error message shows expected vs actual hash

```
Hash mismatch
  File: test.txt
  Line: 2
  Expected hash: 3K
  Actual hash: P2
  Actual content: modified line2
  Hint: Run 'aifed read test.txt' to get current hashes
```

---

## Insert After Line

**Goal:** Verify `+` (insert) operation adds a new line after the specified line.

**Steps:**
```bash
aifed read test.txt
# Insert after line 1 (use its current hash)
aifed edit test.txt <<'EOF'
+ 1:AB "inserted line"
EOF
aifed read test.txt
```

**Expected:**
- New line appears after line 1
- Subsequent line numbers are shifted

```
1:AB|line1
2:5N|inserted line
3:P2|modified line2
4:7M|line3
```

---

## Insert at File Beginning (Virtual Line)

**Goal:** Verify `0:00` virtual line allows insertion at file beginning.

**Steps:**
```bash
aifed edit test.txt <<'EOF'
+ 0:00 "// header"
EOF
aifed read test.txt
```

**Expected:**
- New line appears as line 1
- All existing lines are shifted down

```
1:C8|// header
2:AB|line1
...
```

---

## Delete Line

**Goal:** Verify `-` (delete) operation removes a line when hash matches.

**Steps:**
```bash
aifed read test.txt
# Capture hash for the line to delete
aifed edit test.txt <<'EOF'
- 2:AB
EOF
aifed read test.txt
```

**Expected:**
- Specified line is removed
- Subsequent line numbers are shifted up

```
1:C8|// header
2:5N|inserted line
...
```

---

## Dry Run Preview

**Goal:** Verify `--dry-run` shows what would happen without modifying the file.

**Steps:**
```bash
aifed read test.txt
aifed edit test.txt --dry-run <<'EOF'
- 1:C8
+ 1:C8 "new content"
EOF
aifed read test.txt
```

**Expected:**
- Dry run shows preview message
- File content is unchanged

```
Would apply 2 operations to test.txt
```

---

## Batch Edit via Heredoc

**Goal:** Verify batch mode applies multiple operations atomically via stdin (heredoc).

**Precondition:** A fresh test file.

**Steps:**
```bash
echo -e "line1\nline2\nline3\nline4" > batch.txt
aifed read batch.txt
# Capture hashes for lines, then apply batch edit
aifed edit batch.txt <<EOF
- 1:XX
+ 1:XX "modified line1"
+ 2:XX "inserted after line2"
- 4:XX
EOF
aifed read batch.txt
```

**Expected:**
- All operations are applied
- Line 1 is modified
- New line appears after line 2
- Original line 4 is deleted

---

## Batch Edit Atomicity

**Goal:** Verify batch edit is atomic - if any operation fails, none are applied.

**Steps:**
```bash
echo -e "line1\nline2\nline3" > atomic.txt
aifed read atomic.txt
# Use wrong but syntactically valid hash for second operation
aifed edit atomic.txt <<EOF
- 1:XX
+ 1:XX "this should work"
+ 2:VV "but this hash is wrong"
EOF
aifed read atomic.txt
```

**Expected:**
- Command fails with error
- File is unchanged (line 1 still has original content)

```
Batch parse error on line 3: '+ 2:VV "but this hash is wrong"'
  Reason: Hash mismatch
  File: atomic.txt
  Line: 2
  Expected hash: VV
  Actual hash: TD
  Actual content: line2
  Hint: Run 'aifed read atomic.txt' to get current hashes
```

---

## Batch Edit with Comments

**Goal:** Verify comments and blank lines in batch input are ignored.

**Steps:**
```bash
echo -e "line1\nline2\nline3" > comments.txt
aifed read comments.txt
aifed edit comments.txt <<EOF
# This is a comment
- 1:XX
+ 1:XX "modified"

+ 2:XX "inserted"
# Another comment
EOF
aifed read comments.txt
```

**Expected:**
- Comments and blank lines are ignored
- Only actual operations are executed

---

## JSON Output for Read

**Goal:** Verify `--json` flag outputs valid JSON for programmatic parsing.

**Steps:**
```bash
aifed read test.txt --json
```

**Expected:**
- Output is valid JSON with `lines` array containing `line`, `hash`, and `content` fields

```json
{
  "lines": [
    {
      "line": 1,
      "hash": "AB",
      "content": "line1"
    },
    {
      "line": 2,
      "hash": "3K",
      "content": "line2"
    }
  ]
}
```

---

## JSON Output Without Hashes

**Goal:** Verify `--json --no-hashes` outputs JSON without hash fields.

**Steps:**
```bash
aifed read test.txt --json --no-hashes
```

**Expected:**
- Output is valid JSON with `line` and `content` only (no `hash`)

```json
{
  "lines": [
    {
      "line": 1,
      "content": "line1"
    },
    {
      "line": 2,
      "content": "line2"
    }
  ]
}
```

---

## File Not Found Error

**Goal:** Verify error message for non-existent file.

**Steps:**
```bash
aifed read nonexistent.txt
```

**Expected:**
- Command fails
- Error message indicates file not found

```
File not found: nonexistent.txt
```

---

## Info Command

**Goal:** Verify `aifed info` outputs file metadata correctly.

**Steps:**
```bash
aifed info test.txt
```

**Expected:**
- Output contains file path, line count, and file size

```
Path: test.txt
Lines: <line_count>
Size: <size>
```

---
