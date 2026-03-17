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
aifed read test.txt 1-2
```

**Expected:**
- Output contains lines 1-2 with hashlines

```
1:AB|line1
2:3K|line2
```

---

## Replace Line with Hashline Verification

**Goal:** Verify `=` (replace) operation modifies a line when hash matches.

**Steps:**
```bash
aifed read test.txt
# Capture hash for line 2 (e.g., 3K)
aifed edit test.txt = 2:3K "modified line2"
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

## Replace with Hash Mismatch

**Goal:** Verify edit is rejected when hash does not match current content.

**Steps:**
```bash
# Using stale hash (from original line content)
aifed edit test.txt = 2:3K "should fail"
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
aifed edit test.txt + 1:AB "inserted line"
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
aifed edit test.txt + 0:00 "// header"
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
aifed edit test.txt - 2:AB
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
aifed edit test.txt = 1:C8 "new content" --dry-run
aifed read test.txt
```

**Expected:**
- Dry run shows preview message
- File content is unchanged

```
Would apply = to test.txt
```

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

## Invalid Arguments Error

**Goal:** Verify error for missing required arguments.

**Steps:**
```bash
aifed edit test.txt =
```

**Expected:**
- Command fails
- Error message indicates missing arguments (from clap)

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