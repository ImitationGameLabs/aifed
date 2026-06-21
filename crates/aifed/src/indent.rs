//! Indent directive support for batch edits (`@N`).
//!
//! The `@N` directive lets an agent express indentation relative to an anchor
//! line instead of spelling out leading whitespace, which LLMs count
//! unreliably. `@0` copies the anchor's leading whitespace verbatim; `@+N` /
//! `@-N` adjust by N levels. Only `@±N` needs to know the file's indent unit;
//! `@0` is a pure byte copy and never fails.
//!
//! Detection is consistency-gated: a file is [`IndentKind::Tab`],
//! [`IndentKind::Space`], or [`IndentKind::Unknown`]. On `Unknown`
//! (mixed/undeterminable) `@±N` is refused (hard error) while `@0` keeps
//! working. A project may declare `indent_style` / `indent_width` to skip
//! detection; the declaration is an *assertion* — a contradicting file is also a
//! hard error, never a silent rewrite. See `skill.md` and
//! `docs/reference/edit-commands.md`.

use std::path::Path;

use aifed_common::IndentStyleConfig;
use aifed_common::Registry;

use crate::language::LanguageResolver;

/// Classified indentation of a file (or a forced declaration).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndentKind {
    /// One tab per level.
    Tab,
    /// `width` spaces per level.
    Space { width: u32 },
    /// Indentation could not be turned into a usable unit.
    Unknown(UnknownReason),
}

/// Why detection failed — selects the error message shown for `@±N`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownReason {
    /// Tabs and spaces coexist (across lines or within a single leading run).
    Mixed,
    /// All one style but no consistent width could be confirmed.
    Undeterminable,
}

/// Effective indent situation for a file under resolved settings.
///
/// `config_conflict` is set when the project declared an `indent_style` /
/// `indent_width` that the file contradicts; `@±N` must hard-error in that case
/// rather than silently mixing styles. `@0` ignores both fields (pure copy).
#[derive(Debug, Clone)]
pub struct ResolvedIndent {
    pub kind: IndentKind,
    pub config_conflict: bool,
}

/// Resolved (global + per-language) indent settings, built once per edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentSettings {
    /// Global `[indent] assist` ANDed with a per-language `indent_assist`.
    pub assist_enabled: bool,
    /// Declared style; `None` means detect from the file.
    pub forced_style: Option<IndentStyleConfig>,
    /// Declared spaces per level; `None` means detect. Pairs with `Space`.
    pub forced_width: Option<u32>,
}

impl IndentSettings {
    /// Detection-only defaults: assist on, no forced style/width. Used by
    /// `paste` (which builds ops without directives) and tests.
    pub fn detecting() -> Self {
        Self { assist_enabled: true, forced_style: None, forced_width: None }
    }

    /// Resolve global `[indent]` plus a matching `[[language]]` overlay for the
    /// file's language. Falls back to detection (no overlay) when the file's
    /// extension maps to no configured language.
    pub fn from_registry(reg: &Registry, path: &Path) -> Self {
        let mut assist_enabled = reg.indent().assist;
        let mut forced_style = None;
        let mut forced_width = None;

        let resolver = LanguageResolver::with_overlays(reg.language_overlays());
        if let Some(lang) = resolver.detect(path)
            && let Some(overlay) = reg.language_overlays().iter().find(|o| o.language == lang)
        {
            if let Some(assist) = overlay.indent_assist {
                assist_enabled = assist;
            }
            forced_style = overlay.indent_style;
            forced_width = overlay.indent_width;
        }

        Self { assist_enabled, forced_style, forced_width }
    }
}

/// The leading whitespace run of a line (tabs and spaces only). The rest of the
/// line — including a trailing `\r` for CRLF — is left for callers to handle.
pub fn leading_indent(line: &str) -> &str {
    let end = line
        .bytes()
        .position(|b| b != b' ' && b != b'\t')
        .unwrap_or(line.len());
    // Safe: ' ' and '\t' are single-byte ASCII, so `end` lands on a char
    // boundary (the first byte of whatever follows the run).
    &line[..end]
}

/// Per-file indent analysis shared by [`classify`] and the forced-config check.
struct Analysis {
    has_indented: bool,
    /// Some indented line's run starts with `\t`.
    tab_leading: bool,
    /// Some indented line's run starts with ` `.
    space_leading: bool,
    /// Some run contains a space followed later by a tab (e.g. `"  \t"`).
    mixed_run: bool,
    /// Space counts of all space-leading indented lines.
    space_counts: Vec<u32>,
}

fn analyze(lines: &[String]) -> Analysis {
    let mut a = Analysis {
        has_indented: false,
        tab_leading: false,
        space_leading: false,
        mixed_run: false,
        space_counts: Vec::new(),
    };
    for line in lines {
        // Skip blank lines (no non-whitespace content), including CRLF blanks
        // like "   \r" — `trim` covers \t, \r, and spaces.
        if line.trim().is_empty() {
            continue;
        }
        let run = leading_indent(line);
        if run.is_empty() {
            continue; // content with no indent — not an indented line
        }
        a.has_indented = true;
        let bytes = run.as_bytes();
        if bytes[0] == b'\t' {
            a.tab_leading = true;
        } else {
            a.space_leading = true;
        }
        // A space anywhere before a later tab within the run is a real mix
        // (tab-leading lines may still carry alignment spaces after the tabs;
        // those do not set this flag).
        let mut seen_space = false;
        for &b in bytes {
            if b == b' ' {
                seen_space = true;
            } else if seen_space {
                a.mixed_run = true;
            }
        }
        if bytes[0] == b' ' {
            a.space_counts
                .push(bytes.iter().filter(|&&b| b == b' ').count() as u32);
        }
    }
    a
}

fn classify_from(a: &Analysis) -> IndentKind {
    if !a.has_indented {
        return IndentKind::Unknown(UnknownReason::Undeterminable);
    }
    if a.mixed_run || (a.tab_leading && a.space_leading) {
        return IndentKind::Unknown(UnknownReason::Mixed);
    }
    if a.tab_leading {
        return IndentKind::Tab;
    }
    // Space file: confirm a width from at least two distinct counts, then
    // require every count to be a multiple of the minimum (min-positive +
    // divisibility). A single observation or a non-multiple count (sub-grid
    // alignment) is refused rather than silently snapped.
    // Limitation: this picks the finest consistent width, so a stray shallower line can underestimate the true unit.
    debug_assert!(a.space_leading);
    let mut distinct = a.space_counts.clone();
    distinct.sort_unstable();
    distinct.dedup();
    if distinct.len() < 2 {
        return IndentKind::Unknown(UnknownReason::Undeterminable);
    }
    let width = distinct[0]; // minimum positive count
    if distinct.iter().all(|c| c % width == 0) {
        IndentKind::Space { width }
    } else {
        IndentKind::Unknown(UnknownReason::Undeterminable)
    }
}

/// Classify a file's indentation by detecting from its bytes.
pub fn classify(lines: &[String]) -> IndentKind {
    classify_from(&analyze(lines))
}

/// Resolve the effective indent kind for a file under the given settings.
///
/// Forced `indent_style` / `indent_width` skip detection but assert the file
/// matches; a mismatch sets `config_conflict` (the caller hard-errors on
/// `@±N`). `@0` ignores this entirely.
pub fn resolve(lines: &[String], settings: &IndentSettings) -> ResolvedIndent {
    // Nothing forced -> pure detection, no config conflict possible.
    if settings.forced_style.is_none() {
        return ResolvedIndent { kind: classify(lines), config_conflict: false };
    }

    let a = analyze(lines);
    let detected = classify_from(&a);

    let (kind, config_conflict) = match (settings.forced_style, settings.forced_width) {
        (Some(IndentStyleConfig::Tab), _) => {
            // File must use tabs only (no space-indented line, no intra-run mix).
            (IndentKind::Tab, a.space_leading || a.mixed_run)
        }
        (Some(IndentStyleConfig::Space), Some(w)) => {
            // Config validation only runs on file-loaded entries; IndentSettings
            // can be built directly (overlays, tests), so guard width 0 here too.
            let conflict = w == 0
                || a.tab_leading
                || a.mixed_run
                || a.space_counts.iter().any(|&c| c % w != 0);
            (IndentKind::Space { width: w }, conflict)
        }
        // A style forced without a width cannot assert -> detect.
        _ => (detected, false),
    };

    ResolvedIndent { kind, config_conflict }
}

/// Transform one content line's indentation per the directive.
///
/// `anchor_indent` is the anchor line's [`leading_indent`] run. `@0` copies it
/// verbatim (always works, even for `Unknown` kinds and the empty anchor of the
/// virtual `0:00` line). `@±N` snaps to the level grid; sub-level alignment is
/// not preserved — use `@0` when exact bytes matter. `@-N` floors at column 0.
pub fn apply_indent(
    content: &str,
    anchor_indent: &str,
    directive: i32,
    kind: &IndentKind,
) -> String {
    let body = content.trim_start_matches([' ', '\t']);

    if directive == 0 {
        let mut out = String::with_capacity(anchor_indent.len() + body.len());
        out.push_str(anchor_indent);
        out.push_str(body);
        return out;
    }

    let (anchor_level, unit): (i32, String) = match kind {
        IndentKind::Tab => (anchor_indent.matches('\t').count() as i32, "\t".to_string()),
        IndentKind::Space { width } => {
            let w = *width as usize;
            let spaces = anchor_indent.matches(' ').count();
            let level = if w > 0 { (spaces / w) as i32 } else { 0 };
            (level, " ".repeat(w))
        }
        IndentKind::Unknown(_) => {
            // @+N/@-N is gated by the caller (apply_directive) to a resolvable kind.
            unreachable!("apply_indent called with Unknown kind for a non-zero directive")
        }
    };

    let target = (anchor_level + directive).max(0) as usize;
    let mut out = String::with_capacity(target * unit.len() + body.len());
    for _ in 0..target {
        out.push_str(&unit);
    }
    out.push_str(body);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use IndentKind::*;
    use UnknownReason::*;

    fn lines(input: &[&str]) -> Vec<String> {
        input.iter().map(|s| s.to_string()).collect()
    }

    // ── leading_indent ───────────────────────────────────────────────

    #[test]
    fn leading_indent_stops_at_first_non_whitespace() {
        assert_eq!(leading_indent("\t\tcode"), "\t\t");
        assert_eq!(leading_indent("    code"), "    ");
        assert_eq!(leading_indent("code"), "");
        assert_eq!(leading_indent("\t  bar"), "\t  "); // tab + alignment spaces
        assert_eq!(leading_indent("   \r"), "   "); // CR is not part of the run
        assert_eq!(leading_indent(""), "");
    }

    // ── classify ─────────────────────────────────────────────────────

    #[test]
    fn classify_tab_file() {
        let l = lines(&["fn main() {", "\tlet x = 1;", "\t\tloop {}", "}"]);
        assert_eq!(classify(&l), Tab);
    }

    #[test]
    fn classify_tab_with_alignment_spaces_is_tab() {
        // "tabs for indent, spaces for alignment" must NOT read as mixed.
        let l = lines(&["struct S {", "\tname:      u32,", "\ttimestamp: u64,", "}"]);
        assert_eq!(classify(&l), Tab);
    }

    #[test]
    fn classify_space_file_width_four() {
        let l = lines(&["fn main() {", "    let x = 1;", "        loop {}", "}"]);
        assert_eq!(classify(&l), Space { width: 4 });
    }

    #[test]
    fn classify_space_file_with_deeper_multiple_line() {
        // A line at 8 in a 4-space file is a clean multiple → Space{4}.
        let l = lines(&["a", "    b", "        c"]);
        assert_eq!(classify(&l), Space { width: 4 });
    }

    #[test]
    fn classify_space_file_with_non_multiple_line_is_unknown() {
        // A line at 6 in a 4-space file breaks divisibility → refuse.
        let l = lines(&["a", "    b", "      c"]);
        assert_eq!(classify(&l), Unknown(Undeterminable));
    }

    #[test]
    fn classify_single_indented_line_is_unknown() {
        // One observation cannot confirm a width.
        let l = lines(&["a", "    b"]);
        assert_eq!(classify(&l), Unknown(Undeterminable));
    }

    #[test]
    fn classify_flat_file_is_unknown() {
        let l = lines(&["a", "b", "c"]);
        assert_eq!(classify(&l), Unknown(Undeterminable));
    }

    #[test]
    fn classify_mixed_tab_and_space_lines_is_unknown_mixed() {
        let l = lines(&["a", "\tb", "    c"]);
        assert_eq!(classify(&l), Unknown(Mixed));
    }

    #[test]
    fn classify_space_then_tab_in_run_is_unknown_mixed() {
        let l = lines(&["a", "  \tb"]); // spaces then a tab
        assert_eq!(classify(&l), Unknown(Mixed));
    }

    #[test]
    fn classify_skips_blank_lines() {
        let l = lines(&["a", "", "   ", "    b", "        c", "   \r"]);
        assert_eq!(classify(&l), Space { width: 4 });
    }

    // ── apply_indent ─────────────────────────────────────────────────

    #[test]
    fn apply_zero_copies_anchor_indent() {
        assert_eq!(
            apply_indent("foo", "    ", 0, &Space { width: 4 }),
            "    foo"
        );
        assert_eq!(
            apply_indent("    foo", "    ", 0, &Space { width: 4 }),
            "    foo"
        );
        assert_eq!(apply_indent("foo", "\t\t", 0, &Tab), "\t\tfoo");
    }

    #[test]
    fn apply_zero_on_empty_anchor_is_column_zero() {
        // Virtual line 0:00 has no anchor → @0 yields the body at column 0.
        assert_eq!(apply_indent("foo", "", 0, &Space { width: 4 }), "foo");
        assert_eq!(apply_indent("foo", "", 0, &Unknown(Undeterminable)), "foo");
    }

    #[test]
    fn apply_zero_strips_content_leading_whitespace() {
        assert_eq!(
            apply_indent("\t  foo", "    ", 0, &Space { width: 4 }),
            "    foo"
        );
    }

    #[test]
    fn apply_plus_minus_tab() {
        assert_eq!(apply_indent("x", "\t", 1, &Tab), "\t\tx");
        assert_eq!(apply_indent("x", "\t\t", -1, &Tab), "\tx");
        assert_eq!(apply_indent("x", "\t", 2, &Tab), "\t\t\tx");
    }

    #[test]
    fn apply_plus_minus_space() {
        assert_eq!(
            apply_indent("x", "    ", 1, &Space { width: 4 }),
            "        x"
        );
        assert_eq!(
            apply_indent("x", "        ", -1, &Space { width: 4 }),
            "    x"
        );
        assert_eq!(apply_indent("x", "  ", 1, &Space { width: 2 }), "    x");
    }

    #[test]
    fn apply_negative_floors_at_zero() {
        assert_eq!(apply_indent("x", "\t", -5, &Tab), "x");
        assert_eq!(apply_indent("x", "        ", -3, &Space { width: 4 }), "x");
    }

    #[test]
    fn apply_plus_on_tab_anchor_with_alignment_drops_alignment() {
        // @±N snaps to the tab grid; alignment spaces are not preserved.
        assert_eq!(apply_indent("x", "\t  ", 1, &Tab), "\t\tx");
    }

    #[test]
    fn apply_whitespace_only_content_yields_indent_only() {
        assert_eq!(apply_indent("    ", "    ", 0, &Space { width: 4 }), "    ");
        assert_eq!(apply_indent("    ", "\t", 1, &Tab), "\t\t");
    }

    #[test]
    fn apply_preserves_trailing_cr() {
        assert_eq!(
            apply_indent("foo\r", "    ", 0, &Space { width: 4 }),
            "    foo\r"
        );
    }

    // ── resolve (forced config as assertion) ─────────────────────────

    fn settings(style: Option<IndentStyleConfig>, width: Option<u32>) -> IndentSettings {
        IndentSettings { assist_enabled: true, forced_style: style, forced_width: width }
    }

    #[test]
    fn resolve_forced_tab_consistent() {
        let l = lines(&["a", "\tb", "\t\tc"]);
        let r = resolve(&l, &settings(Some(IndentStyleConfig::Tab), None));
        assert_eq!(r.kind, Tab);
        assert!(!r.config_conflict);
    }

    #[test]
    fn resolve_forced_tab_conflicts_with_space_file() {
        let l = lines(&["a", "    b", "        c"]);
        let r = resolve(&l, &settings(Some(IndentStyleConfig::Tab), None));
        assert!(r.config_conflict);
    }

    #[test]
    fn resolve_forced_space_consistent() {
        let l = lines(&["a", "    b", "        c"]);
        let r = resolve(&l, &settings(Some(IndentStyleConfig::Space), Some(4)));
        assert_eq!(r.kind, Space { width: 4 });
        assert!(!r.config_conflict);
    }

    #[test]
    fn resolve_forced_space_conflicts_with_tab_file() {
        let l = lines(&["a", "\tb"]);
        let r = resolve(&l, &settings(Some(IndentStyleConfig::Space), Some(4)));
        assert!(r.config_conflict);
    }

    #[test]
    fn resolve_forced_space_conflicts_with_non_multiple_width() {
        let l = lines(&["a", "    b", "      c"]); // 6 is not a multiple of 4
        let r = resolve(&l, &settings(Some(IndentStyleConfig::Space), Some(4)));
        assert!(r.config_conflict);
    }

    #[test]
    fn resolve_detects_when_nothing_forced() {
        let l = lines(&["a", "    b", "        c"]);
        let r = resolve(&l, &settings(None, None));
        assert_eq!(r.kind, Space { width: 4 });
        assert!(!r.config_conflict);
    }

    #[test]
    fn resolve_forced_space_without_width_detects() {
        let l = lines(&["a", "    b", "        c"]);
        let r = resolve(&l, &settings(Some(IndentStyleConfig::Space), None));
        assert_eq!(r.kind, Space { width: 4 });
        assert!(!r.config_conflict);
    }

    #[test]
    fn resolve_forced_space_zero_width_is_conflict() {
        // Space-indented input keeps space_counts non-empty so the c % w path runs;
        // width 0 must short-circuit to a conflict without dividing by zero.
        let l = lines(&["a", "    b", "        c"]);
        let r = resolve(&l, &settings(Some(IndentStyleConfig::Space), Some(0)));
        assert!(r.config_conflict);
    }
}
