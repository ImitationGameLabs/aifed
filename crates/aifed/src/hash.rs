use xxhash_rust::xxh64;

/// base32hex character set: 0-9, A-V
const BASE32HEX_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHIJKLMNOPQRSTUV";

/// Hash a single line of content.
///
/// Uses xxHash64, takes the top 10 bits, and encodes as 2-character base32hex.
/// Whitespace is preserved (not stripped).
pub fn hash_line(content: &str) -> String {
    let hash = xxh64::xxh64(content.as_bytes(), 0);
    let bits10 = (hash >> 54) as u16;
    base32hex_encode(bits10)
}

/// Encode a 10-bit value as 2-character base32hex string.
fn base32hex_encode(value: u16) -> String {
    let hi = (value >> 5) as usize;
    let lo = (value & 0x1F) as usize;
    format!("{}{}", BASE32HEX_ALPHABET[hi] as char, BASE32HEX_ALPHABET[lo] as char)
}

/// Virtual line hash constant for inserting at the beginning of a file.
pub const VIRTUAL_LINE_HASH: &str = "00";

/// Check if the given hash is the virtual line hash.
pub fn is_virtual_hash(hash: &str) -> bool {
    hash == VIRTUAL_LINE_HASH
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_line_consistent() {
        let content = "fn main() {";
        let hash1 = hash_line(content);
        let hash2 = hash_line(content);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 2);
    }

    #[test]
    fn test_hash_line_whitespace_sensitive() {
        // Whitespace is now preserved, so different whitespace = different hash
        let hash1 = hash_line("fn main() {");
        let hash2 = hash_line("fn  main ( )  { ");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_line_whitespace_preserved() {
        // Same content with same whitespace = same hash
        let hash1 = hash_line("fn main() {");
        let hash2 = hash_line("fn main() {");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_line_different_content() {
        let hash1 = hash_line("fn main() {");
        let hash2 = hash_line("fn other() {");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_line_base32hex_chars() {
        let hash = hash_line("test");
        // Should only contain base32hex characters
        assert!(hash.chars().all(|c| c.is_ascii_digit() || ('A'..='V').contains(&c)));
    }

    #[test]
    fn test_virtual_hash() {
        assert!(is_virtual_hash("00"));
        assert!(!is_virtual_hash("AB"));
    }
}
