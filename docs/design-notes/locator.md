# Locator Design

## Deterministic Positioning (Hashline)

**Problem:** Line numbers drift when files change; concurrent edits cause unexpected modifications.

**Solution:** Hashline - line number + content hash combination

| Approach  | Safety            | Human-readable | Debug-friendly   |
| --------- | ----------------- | -------------- | ---------------- |
| line+hash | ✅ Dual validation | ✅ Line visible | ✅ Easy to locate |
| hash only | ✅ Content-based   | ❌ Need lookup  | ⚠️ Requires grep  |
| line only | ❌ Drift risk      | ✅ Direct       | ✅ Direct         |

See the [Locator Reference](../reference/locator.md) for usage details.

## Hash Algorithm

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
