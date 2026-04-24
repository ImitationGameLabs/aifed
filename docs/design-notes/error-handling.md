# Error Handling

## Error Format

```
Error: Hash mismatch
  File: main.rs
  Expected hash: AB
  Actual hash: 3K
  Actual content: fn main() {
  Hint: Run 'aifed read main.rs' to get current hashes
```

## No Exit Codes

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
