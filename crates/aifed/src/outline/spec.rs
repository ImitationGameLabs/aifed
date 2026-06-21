//! Per-language configuration driving the generic outline walker.
//!
//! Each language implements [`Spec`] as a unit struct: a [`Spec::language`] +
//! [`Spec::grammar_name`] pair (so the shared [`super::walker::extract`] can set
//! up the parser), a [`Spec::expand`] hook (node fan-out for containers like Go
//! `type_declaration` or C multi-declarators; defaults to the node itself), a
//! [`Spec::classify`] hook (node -> kind/name/body), and a [`Spec::doc_policy`]
//! (doc/attribute/decorator attribution).

use tree_sitter::{Language, Node};

use super::model::ItemKind;

/// One classified node: its outline kind, display name, optional body to
/// recurse into, and optional auxiliary detail. Returned by [`Spec::classify`].
pub struct Classified<'a> {
    pub kind: ItemKind,
    pub name: String,
    /// Recurse into this node for nested children; `None` for a leaf item.
    pub body: Option<Node<'a>>,
    /// Auxiliary text shown after the name (e.g. a Go method receiver). `None`
    /// for items without detail; Rust never sets it.
    pub detail: Option<String>,
    /// Hidden unless the caller's `imports` flag is set (e.g. Rust `use`).
    pub imports_gated: bool,
}

/// Doc-comment / attribute / decorator attribution policy.
///
/// Generalizes Rust's rule (`attribute_item` siblings + outer `///`/`/**` doc
/// comments, blank-line break) to all languages. See `effective_start_line` in
/// [`super::walker`] for the exact join.
///
/// Note: when `expand_backward` is `false` (Python, whose docstrings live in the
/// body rather than above the item) the walker skips the backward walk entirely,
/// so the other fields are ignored.
#[derive(Clone, Copy)]
pub struct DocPolicy {
    /// Named sibling kinds that attach even though they aren't comments
    /// (Rust: `["attribute_item"]`; most languages: empty).
    pub attribute_kinds: &'static [&'static str],
    /// Named sibling kinds that attach as decorators whose range extends back
    /// over the item (most languages: empty).
    ///
    /// NOTE: this is only consulted on the backward `prev_sibling` walk, which
    /// `expand_backward = false` skips entirely, so it is unreachable in that
    /// mode. The Python decorator design (plan Phase 2) is unresolved: a
    /// `decorator` is a *named child* of `decorated_definition`, not a
    /// prev-sibling of the unwrapped definition, so decorators are likely best
    /// handled by `expand` returning the inner node from
    /// `decorated_definition` (its pre-expand container becomes the doc anchor)
    /// rather than via this field. Revisit when Python lands.
    pub decorator_kinds: &'static [&'static str],
    /// Treat `is_extra()` siblings (comments) as attachment candidates.
    pub attach_extras: bool,
    /// When non-empty, an extra sibling attaches only when its text starts with
    /// one of these prefixes (e.g. Rust `///`/`/**`). When **empty** +
    /// `attach_extras`, EVERY extra attaches — any contiguous preceding comment
    /// is treated as a doc (Go/C/C++/JS/TS). See each language's `DocPolicy`.
    pub doc_prefixes: &'static [&'static str],
    /// Walk `prev_sibling` backward at all. False for Python (docstrings in-body).
    pub expand_backward: bool,
    /// A blank source line in the gap breaks the run.
    pub blank_line_breaks: bool,
}

/// Per-language outline specification.
///
/// Implement as a unit struct (e.g. `pub struct RustSpec;`); the shared
/// [`super::walker::extract`] drives parsing and traversal from `language()`,
/// `classify()`, and `doc_policy()`.
pub trait Spec {
    /// The tree-sitter grammar for this language.
    fn language(&self) -> Language;

    /// Grammar name for error messages (e.g. `"rust"`).
    fn grammar_name(&self) -> &'static str;

    /// Fan a node out into the nodes to classify. Default: the node itself.
    /// Override to drill containers (Go `type_declaration` -> its `type_spec`
    /// children; C `declaration` -> its struct/enum/union specifier or a prototype;
    /// pierce `preproc_if`/`#ifdef`/`#else`/`#elif` recursively).
    /// Doc attribution always uses the pre-expand container, so an override only
    /// changes *which nodes get classified*, not where their docs attach.
    fn expand<'a>(&self, node: Node<'a>) -> Vec<Node<'a>> {
        vec![node]
    }

    /// Classify one (already-expanded) node, or `None` to skip it (comments,
    /// standalone attributes, `ERROR`/`MISSING`, unrecognized kinds).
    fn classify<'a>(&self, node: Node<'a>, source: &str) -> Option<Classified<'a>>;

    /// The doc/attribute attribution policy for this language (a compile-time
    /// constant).
    fn doc_policy(&self) -> &'static DocPolicy;
}
