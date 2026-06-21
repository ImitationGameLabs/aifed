//! Shared test helpers for the per-language outline `Spec`s.
//!
//! Compiled only under `cfg(test)` (the module is declared `#[cfg(test)]` in
//! the parent), so it carries no production weight.

use crate::outline::model::OutlineItem;

/// Find the first item named `name` in `items`, panicking with a clear
/// message if it is absent. Used by every language spec's inline tests.
pub fn find<'a>(items: &'a [OutlineItem], name: &str) -> &'a OutlineItem {
    items
        .iter()
        .find(|i| i.name == name)
        .unwrap_or_else(|| panic!("item '{name}' not found in {items:?}"))
}
