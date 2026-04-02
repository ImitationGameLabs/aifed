use crate::error::{Error, Result};

/// Locator types for positioning in files
#[derive(Debug, Clone, PartialEq)]
pub enum Locator {
    /// Hashline: line number + content hash (e.g., "42:AB")
    Hashline { line: usize, hash: String },
    /// Single line: just a line number (e.g., "42")
    Line(usize),
    /// Line range: start and end line numbers (e.g., "[10,20]")
    LineRange { start: usize, end: usize },
    /// Hashline range: start/end lines with hashes for range delete (e.g., "[2:AA,89:BB]")
    HashlineRange { start: usize, start_hash: String, end: usize, end_hash: String },
}

impl Locator {
    /// Parse a locator string.
    ///
    /// Formats:
    /// - "42:AB" -> Hashline { line: 42, hash: "AB" }
    /// - "42" -> Line(42)
    /// - "[10,20]" -> LineRange { start: 10, end: 20 }
    /// - "[2:AA,89:BB]" -> HashRange { start: 2, start_hash: "AA", end: 89, end_hash: "BB" }
    pub fn parse(input: &str) -> Result<Self> {
        let input = input.trim();

        // Check for bracket format: [LEFT,RIGHT]
        if input.starts_with('[') && input.ends_with(']') {
            let inner = &input[1..input.len() - 1];
            let (left, right) = inner.split_once(',').ok_or_else(|| Error::InvalidLocator {
                input: input.to_string(),
                reason: "Bracket format must have comma separator".to_string(),
            })?;

            let left_loc = Self::parse(left.trim())?;
            let right_loc = Self::parse(right.trim())?;

            // Determine if this is LineRange or HashlineRange
            match (left_loc, right_loc) {
                (Locator::Line(start), Locator::Line(end)) => {
                    // Line range: [10,20]
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
                    return Ok(Locator::LineRange { start, end });
                }
                (
                    Locator::Hashline { line: start, hash: start_hash },
                    Locator::Hashline { line: end, hash: end_hash },
                ) => {
                    // Hashline range: [2:AA,89:BB]
                    if start > end {
                        return Err(Error::InvalidLocator {
                            input: input.to_string(),
                            reason: "Hash range start cannot be greater than end".to_string(),
                        });
                    }
                    return Ok(Locator::HashlineRange { start, start_hash, end, end_hash });
                }
                _ => {
                    return Err(Error::InvalidLocator {
                        input: input.to_string(),
                        reason:
                            "Bracket format boundaries must be both line numbers or both hashlines"
                                .to_string(),
                    });
                }
            }
        }

        // Check for hashline format (e.g., "42:AB")
        if let Some((line_str, hash)) = input.split_once(':') {
            let line: usize = line_str.parse().map_err(|_| Error::InvalidLocator {
                input: input.to_string(),
                reason: format!("Invalid line number: {}", line_str),
            })?;

            // Validate hash is base32hex (0-9, A-V)
            if !hash
                .chars()
                .all(|c| c.is_ascii_digit() || ('A'..='V').contains(&c) || ('a'..='v').contains(&c))
            {
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
            reason: "Expected line number, range [START,END], or hashline LINE:HASH".to_string(),
        })?;

        Ok(Locator::Line(line))
    }

    /// Get the primary line number (first line for ranges).
    /// Returns None for line 0 (virtual line).
    #[allow(dead_code)]
    pub fn line(&self) -> Option<usize> {
        match self {
            Locator::Hashline { line, .. } if *line > 0 => Some(*line),
            Locator::Line(line) if *line > 0 => Some(*line),
            Locator::LineRange { start, .. } => Some(*start),
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
            Locator::Line(line) => write!(f, "{}", line),
            Locator::LineRange { start, end } => write!(f, "[{},{}]", start, end),
            Locator::HashlineRange { start, start_hash, end, end_hash } => {
                write!(f, "[{}:{},{}:{}]", start, start_hash, end, end_hash)
            }
        }
    }
}

/// Symbol locator for positioning within a line
/// Format: "S<index>:<name>" (e.g., "S1:config")
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolLocator {
    /// 1-based index of the symbol on the line
    pub index: u32,
    /// Name of the symbol
    pub name: String,
}

impl SymbolLocator {
    /// Parse a symbol locator string.
    ///
    /// Format: "S<index>:<name>" (e.g., "S1:config")
    pub fn parse(input: &str) -> Result<Self> {
        let input = input.trim();

        // Must start with 'S' or 's'
        let rest = input
            .strip_prefix('S')
            .or_else(|| input.strip_prefix('s'))
            .ok_or_else(|| Error::InvalidLocator {
                input: input.to_string(),
                reason: "Symbol locator must start with 'S'".to_string(),
            })?;

        // Split on ':'
        let (index_str, name) = rest.split_once(':').ok_or_else(|| Error::InvalidLocator {
            input: input.to_string(),
            reason: "Symbol locator must be in format 'S<index>:<name>'".to_string(),
        })?;

        let index: u32 = index_str.parse().map_err(|_| Error::InvalidLocator {
            input: input.to_string(),
            reason: format!("Invalid symbol index: {}", index_str),
        })?;

        if index == 0 {
            return Err(Error::InvalidLocator {
                input: input.to_string(),
                reason: "Symbol index must be at least 1".to_string(),
            });
        }

        if name.is_empty() {
            return Err(Error::InvalidLocator {
                input: input.to_string(),
                reason: "Symbol name cannot be empty".to_string(),
            });
        }

        // Validate name is a valid identifier
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '!' || c == '?')
        {
            return Err(Error::InvalidLocator {
                input: input.to_string(),
                reason: "Symbol name must be a valid identifier".to_string(),
            });
        }

        Ok(SymbolLocator { index, name: name.to_string() })
    }

    /// Find the character offset of this symbol in the given line content.
    /// Returns the 0-based character offset, or None if not found.
    pub fn find_offset(&self, line_content: &str) -> Option<u32> {
        let mut count = 0u32;
        let mut pos = 0usize;

        while pos < line_content.len() {
            // Skip non-identifier characters
            while pos < line_content.len()
                && !line_content[pos..].starts_with(|c: char| c.is_alphabetic() || c == '_')
            {
                // Handle UTF-8 properly
                pos += line_content[pos..]
                    .char_indices()
                    .next()
                    .map(|(_, c)| c.len_utf8())
                    .unwrap_or(1);
            }

            if pos >= line_content.len() {
                break;
            }

            // Find end of identifier
            let start = pos;
            while pos < line_content.len() {
                let ch = line_content[pos..].chars().next().unwrap();
                if ch.is_alphanumeric() || ch == '_' || ch == '!' || ch == '?' {
                    pos += ch.len_utf8();
                } else {
                    break;
                }
            }

            let ident = &line_content[start..pos];
            count += 1;

            if count == self.index && ident == self.name {
                return Some(start as u32);
            }
        }

        None
    }
}

impl std::fmt::Display for SymbolLocator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "S{}:{}", self.index, self.name)
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
    fn test_parse_line() {
        let loc = Locator::parse("42").unwrap();
        assert_eq!(loc, Locator::Line(42));
    }

    #[test]
    fn test_parse_line_range() {
        let loc = Locator::parse("[10,20]").unwrap();
        assert_eq!(loc, Locator::LineRange { start: 10, end: 20 });
    }

    #[test]
    fn test_parse_line_range_with_spaces() {
        let loc = Locator::parse("[ 10 , 20 ]").unwrap();
        assert_eq!(loc, Locator::LineRange { start: 10, end: 20 });
    }

    #[test]
    fn test_parse_virtual_line() {
        let loc = Locator::parse("0:00").unwrap();
        assert!(loc.is_virtual());
    }

    #[test]
    fn test_parse_invalid_range() {
        let result = Locator::parse("[20,10]");
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
        assert_eq!(Locator::parse("[10,20]").unwrap().line(), Some(10));
        assert_eq!(Locator::parse("0:00").unwrap().line(), None);
    }

    // SymbolLocator tests

    #[test]
    fn test_parse_symbol_locator() {
        let loc = SymbolLocator::parse("S1:config").unwrap();
        assert_eq!(loc.index, 1);
        assert_eq!(loc.name, "config");
    }

    #[test]
    fn test_parse_symbol_locator_lowercase() {
        let loc = SymbolLocator::parse("s2:main").unwrap();
        assert_eq!(loc.index, 2);
        assert_eq!(loc.name, "main");
    }

    #[test]
    fn test_parse_symbol_locator_invalid() {
        assert!(SymbolLocator::parse("1:config").is_err());
        assert!(SymbolLocator::parse("S0:name").is_err());
        assert!(SymbolLocator::parse("S1:").is_err());
        assert!(SymbolLocator::parse("S:name").is_err());
    }

    #[test]
    fn test_symbol_find_offset() {
        let line = "let config = load_config();";
        let loc = SymbolLocator::parse("S1:let").unwrap();
        assert_eq!(loc.find_offset(line), Some(0));

        let loc = SymbolLocator::parse("S2:config").unwrap();
        assert_eq!(loc.find_offset(line), Some(4));

        let loc = SymbolLocator::parse("S3:load_config").unwrap();
        assert_eq!(loc.find_offset(line), Some(13));

        // Not found
        let loc = SymbolLocator::parse("S99:foo").unwrap();
        assert_eq!(loc.find_offset(line), None);
    }

    // HashlineRange tests

    #[test]
    fn test_parse_hashline_range() {
        let loc = Locator::parse("[2:AA,89:BB]").unwrap();
        assert_eq!(
            loc,
            Locator::HashlineRange {
                start: 2,
                start_hash: "AA".to_string(),
                end: 89,
                end_hash: "BB".to_string()
            }
        );
    }

    #[test]
    fn test_parse_hashline_range_whitespace() {
        let loc = Locator::parse("[ 2:AA , 89:BB ]").unwrap();
        assert_eq!(
            loc,
            Locator::HashlineRange {
                start: 2,
                start_hash: "AA".to_string(),
                end: 89,
                end_hash: "BB".to_string()
            }
        );
    }

    #[test]
    fn test_parse_hashline_range_single_line() {
        // Same start and end line
        let loc = Locator::parse("[3:AB,3:AB]").unwrap();
        assert_eq!(
            loc,
            Locator::HashlineRange {
                start: 3,
                start_hash: "AB".to_string(),
                end: 3,
                end_hash: "AB".to_string()
            }
        );
    }

    #[test]
    fn test_parse_hashline_range_invalid_start_gt_end() {
        let result = Locator::parse("[89:BB,2:AA]");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_hashline_range_invalid_format() {
        // Missing brackets
        assert!(Locator::parse("2:AA,89:BB]").is_err());
        assert!(Locator::parse("[2:AA,89:BB").is_err());
        // Only one hashline
        assert!(Locator::parse("[2:AA]").is_err());
        // No comma
        assert!(Locator::parse("[2:AA 89:BB]").is_err());
    }

    #[test]
    fn test_parse_hashline_range_invalid_hash() {
        // Invalid hash characters (X, Y, Z not in base32hex)
        assert!(Locator::parse("[2:XX,89:YY]").is_err());
    }
}
