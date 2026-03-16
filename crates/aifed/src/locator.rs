use crate::error::{Error, Result};

/// Locator types for positioning in files
#[derive(Debug, Clone, PartialEq)]
pub enum Locator {
    /// Hashline: line number + content hash (e.g., "42:AB")
    Hashline { line: usize, hash: String },
    /// Line only: just a line number (e.g., "42")
    LineOnly(usize),
    /// Range: start and end line numbers (e.g., "10-20")
    Range { start: usize, end: usize },
}

impl Locator {
    /// Parse a locator string.
    ///
    /// Formats:
    /// - "42:AB" -> Hashline { line: 42, hash: "AB" }
    /// - "42" -> LineOnly(42)
    /// - "10-20" -> Range { start: 10, end: 20 }
    pub fn parse(input: &str) -> Result<Self> {
        let input = input.trim();

        // Check for range format first (e.g., "10-20")
        if let Some((start_str, end_str)) = input.split_once('-') {
            let start: usize = start_str.parse().map_err(|_| Error::InvalidLocator {
                input: input.to_string(),
                reason: format!("Invalid range start: {}", start_str),
            })?;
            let end: usize = end_str.parse().map_err(|_| Error::InvalidLocator {
                input: input.to_string(),
                reason: format!("Invalid range end: {}", end_str),
            })?;

            if start > end {
                return Err(Error::InvalidLocator {
                    input: input.to_string(),
                    reason: "Range start cannot be greater than end".to_string(),
                });
            }

            if start == 0 {
                return Err(Error::InvalidLocator {
                    input: input.to_string(),
                    reason: "Range start must be at least 1".to_string(),
                });
            }

            return Ok(Locator::Range { start, end });
        }

        // Check for hashline format (e.g., "42:AB")
        if let Some((line_str, hash)) = input.split_once(':') {
            let line: usize = line_str.parse().map_err(|_| Error::InvalidLocator {
                input: input.to_string(),
                reason: format!("Invalid line number: {}", line_str),
            })?;

            // Validate hash is base32hex (0-9, A-V)
            if !hash.chars().all(|c| c.is_ascii_digit() || ('A'..='V').contains(&c) || ('a'..='v').contains(&c)) {
                return Err(Error::InvalidLocator {
                    input: input.to_string(),
                    reason: "Hash must be base32hex characters (0-9, A-V)".to_string(),
                });
            }

            return Ok(Locator::Hashline { line, hash: hash.to_uppercase() });
        }

        // Must be line-only format (e.g., "42")
        let line: usize = input.parse().map_err(|_| Error::InvalidLocator {
            input: input.to_string(),
            reason: "Expected line number, range (START-END), or hashline (LINE:HASH)".to_string(),
        })?;

        Ok(Locator::LineOnly(line))
    }

    /// Get the primary line number (first line for ranges).
    /// Returns None for line 0 (virtual line).
    #[allow(dead_code)]
    pub fn line(&self) -> Option<usize> {
        match self {
            Locator::Hashline { line, .. } if *line > 0 => Some(*line),
            Locator::LineOnly(line) if *line > 0 => Some(*line),
            Locator::Range { start, .. } => Some(*start),
            _ => None,
        }
    }

    /// Check if this is the virtual line (0:00)
    pub fn is_virtual(&self) -> bool {
        match self {
            Locator::Hashline { line, hash } => *line == 0 && hash == "00",
            _ => false,
        }
    }
}

impl std::fmt::Display for Locator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Locator::Hashline { line, hash } => write!(f, "{}:{}", line, hash),
            Locator::LineOnly(line) => write!(f, "{}", line),
            Locator::Range { start, end } => write!(f, "{}-{}", start, end),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hashline() {
        let loc = Locator::parse("42:AB").unwrap();
        assert_eq!(loc, Locator::Hashline { line: 42, hash: "AB".to_string() });
    }

    #[test]
    fn test_parse_hashline_normalizes_case() {
        let loc = Locator::parse("42:ab").unwrap();
        assert_eq!(loc, Locator::Hashline { line: 42, hash: "AB".to_string() });
    }

    #[test]
    fn test_parse_line_only() {
        let loc = Locator::parse("42").unwrap();
        assert_eq!(loc, Locator::LineOnly(42));
    }

    #[test]
    fn test_parse_range() {
        let loc = Locator::parse("10-20").unwrap();
        assert_eq!(loc, Locator::Range { start: 10, end: 20 });
    }

    #[test]
    fn test_parse_virtual_line() {
        let loc = Locator::parse("0:00").unwrap();
        assert!(loc.is_virtual());
    }

    #[test]
    fn test_parse_invalid_range() {
        let result = Locator::parse("20-10");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_hash() {
        let result = Locator::parse("42:XYZ!!!");
        assert!(result.is_err());
    }

    #[test]
    fn test_line_method() {
        assert_eq!(Locator::parse("42:AB").unwrap().line(), Some(42));
        assert_eq!(Locator::parse("42").unwrap().line(), Some(42));
        assert_eq!(Locator::parse("10-20").unwrap().line(), Some(10));
        assert_eq!(Locator::parse("0:00").unwrap().line(), None);
    }
}
