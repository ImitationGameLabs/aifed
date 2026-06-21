//! Data model for the `outline` command.
//!
//! Pure data: the [`Outline`] tree and [`ItemKind`] categories, each carrying
//! 1-based inclusive line ranges that copy-paste into `aifed read <FILE>
//! [start,end]`. All presentation lives in [`render`](super::render) /
//! [`output::format_outline`](crate::output::format_outline).

use serde::Serialize;

/// The structural category of an outline entry.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum ItemKind {
    Module,
    Function,
    Struct,
    Class,
    Enum,
    Union,
    Trait,
    Interface,
    Impl,
    TypeAlias,
    Const,
    Variable,
    Static,
    Macro,
    /// `extern "ABI" { ... }` foreign module.
    Extern,
    /// Markdown heading.
    Heading,
    /// A `use` import (only emitted with `--imports`).
    Imports,
    /// Synthetic region covering everything before the first definition (leading
    /// `mod`/`use` declarations, inner docs, comments). Not a symbol — rendered
    /// as a self-documenting `file header` row and excluded from the item count.
    FileHeader,
}

/// `serde` predicate: omit `has_body` from JSON when false (the common case).
fn is_false(b: &bool) -> bool {
    !b
}

/// A single outline node with a 1-based inclusive line range.
#[derive(Debug, Clone, Serialize)]
pub struct OutlineItem {
    pub kind: ItemKind,
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
    /// Whether this item carries a body we recurse into (`impl`/`trait`/
    /// `mod { .. }`/`extern { .. }`). Distinguishes a bodyless `mod foo;`
    /// forward declaration from an inline `mod foo {}` so the preamble fold can
    /// target only the former.
    ///
    /// Derived in the walker as `has_body == classified.body.is_some()` — i.e.
    /// "had a recurse-target at all", not "has children": an empty inline
    /// `mod foo {}` has a body but zero children.
    #[serde(skip_serializing_if = "is_false", default)]
    pub has_body: bool,
    /// Markdown heading depth (1-6); absent for code items.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    /// Auxiliary display info (e.g. a code block's language).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<OutlineItem>,
}

/// The full outline of a file.
#[derive(Debug, Serialize)]
pub struct Outline {
    pub path: String,
    pub language: &'static str,
    pub total_lines: usize,
    pub items: Vec<OutlineItem>,
}
