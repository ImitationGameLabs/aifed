//! Python outline extraction via tree-sitter.
//!
//! Drives the generic [`super::walker`] via [`PythonSpec`]. Python docstrings
//! live in the body (the first string statement), not above the item, so the
//! doc policy sets `expand_backward: false` and never walks back. A decorated
//! def/class is wrapped in a `decorated_definition`; [`Spec::expand`] unwraps it
//! to the inner definition, and because doc attribution uses the pre-expand
//! container, the `@decorator` lines are included in the item's start range.
//! Functions and classes recurse into their `body` (Python nests definitions).

use std::path::Path;

use tree_sitter::{Language, Node};

use super::helpers::{field_text, find_child_by_kind};
use super::model::{ItemKind, OutlineItem};
use super::spec::{Classified, DocPolicy, Spec};
use super::walker;
use crate::error::Result;

/// Extract a Python source outline. `imports` controls whether `import` items
/// appear.
pub fn extract(source: &str, imports: bool, path: &Path) -> Result<Vec<OutlineItem>> {
    walker::extract(&PythonSpec, source, imports, path)
}

/// Per-language spec driving the generic walker for Python.
pub struct PythonSpec;

/// Doc attribution for Python: docstrings are in-body, so the backward walk is
/// disabled entirely (the remaining fields are then unused). The `@decorator`
/// lines of a `decorated_definition` are captured via the pre-expand container,
/// not via this policy.
const PYTHON_DOC_POLICY: DocPolicy = DocPolicy {
    attribute_kinds: &[],
    decorator_kinds: &[],
    attach_extras: false,
    doc_prefixes: &[],
    expand_backward: false,
    blank_line_breaks: false,
};

impl Spec for PythonSpec {
    fn language(&self) -> Language {
        tree_sitter_python::LANGUAGE.into()
    }

    fn grammar_name(&self) -> &'static str {
        "python"
    }

    /// Unwrap a `decorated_definition` to its inner `definition` (the actual
    /// fn/class); everything else fans out to itself.
    fn expand<'a>(&self, node: Node<'a>) -> Vec<Node<'a>> {
        if node.kind() == "decorated_definition"
            && let Some(inner) = node.child_by_field_name("definition")
        {
            return vec![inner];
        }
        vec![node]
    }

    fn classify<'a>(&self, node: Node<'a>, source: &str) -> Option<Classified<'a>> {
        let (kind, name, body) = match node.kind() {
            "function_definition" => (
                ItemKind::Function,
                field_text(node, "name", source),
                node.child_by_field_name("body"),
            ),
            "class_definition" => (
                ItemKind::Class,
                field_text(node, "name", source),
                node.child_by_field_name("body"),
            ),
            // `left` is a `type` node; its text is the alias name (e.g. `Foo` or
            // `Foo[T]` for a generic alias).
            "type_alias_statement" => (ItemKind::TypeAlias, field_text(node, "left", source), None),
            "import_statement" | "import_from_statement" | "future_import_statement" => {
                (ItemKind::Imports, import_name(node, source), None)
            }
            _ => return None,
        };
        if name.is_empty() {
            return None;
        }
        let imports_gated = matches!(kind, ItemKind::Imports);
        Some(Classified { kind, name, body, detail: None, imports_gated })
    }

    fn doc_policy(&self) -> &'static DocPolicy {
        &PYTHON_DOC_POLICY
    }
}

/// Module of an import: the `module_name` field for `from`/`__future__` imports
/// (e.g. `os.path` or a relative `.pkg`); otherwise the first `dotted_name`
/// (`import os`). Empty when neither is present.
fn import_name(node: Node<'_>, source: &str) -> String {
    if let Some(module) = node.child_by_field_name("module_name") {
        return module
            .utf8_text(source.as_bytes())
            .map(|t| t.trim().to_string())
            .unwrap_or_default();
    }
    find_child_by_kind(node, "dotted_name")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .map(|t| t.trim().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outline::test_support::find;
    use std::path::Path;

    fn extract(src: &str, imports: bool) -> Vec<OutlineItem> {
        super::extract(src, imports, Path::new("test.py")).unwrap()
    }

    #[test]
    fn top_level_kinds_and_names() {
        let src =
            "def add(a, b):\n    return a + b\n\nclass Point:\n    pass\n\ntype Vec = list[int]\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "add").kind, ItemKind::Function);
        assert_eq!(find(&items, "Point").kind, ItemKind::Class);
        assert_eq!(find(&items, "Vec").kind, ItemKind::TypeAlias);
    }

    #[test]
    fn nested_definition_inside_function() {
        let src = "def outer():\n    def inner():\n        pass\n    return inner\n";
        let items = extract(src, false);
        let outer = find(&items, "outer");
        assert_eq!(
            outer
                .children
                .iter()
                .find(|c| c.name == "inner")
                .map(|c| c.kind),
            Some(ItemKind::Function)
        );
    }

    #[test]
    fn class_methods_recurse() {
        let src = "class C:\n    def method(self):\n        pass\n";
        let items = extract(src, false);
        let c = find(&items, "C");
        assert_eq!(
            c.children
                .iter()
                .find(|m| m.name == "method")
                .map(|m| m.kind),
            Some(ItemKind::Function)
        );
    }

    #[test]
    fn decorator_lines_included_in_range() {
        // `@deco` (line 1) wraps the def in a decorated_definition; the item's
        // start is the container's start (line 1), so the decorator is included.
        let src = "@deco\ndef foo():\n    pass\n";
        let items = extract(src, false);
        let foo = find(&items, "foo");
        assert_eq!((foo.start_line, foo.end_line), (1, 3));
    }

    #[test]
    fn docstring_does_not_pull_start_backward() {
        // expand_backward is false, so the def's own line (1) is the start — the
        // docstring inside the body never extends the range upward.
        let src = "def foo():\n    \"\"\"doc\"\"\"\n    pass\n";
        let items = extract(src, false);
        let foo = find(&items, "foo");
        assert_eq!(foo.start_line, 1);
    }

    #[test]
    fn imports_hidden_by_default_shown_with_flag() {
        let src = "import os.path\ndef main():\n    pass\n";
        assert!(
            extract(src, false)
                .iter()
                .all(|i| i.kind != ItemKind::Imports),
            "import hidden without --imports"
        );
        let with_imports = extract(src, true);
        assert_eq!(
            with_imports
                .iter()
                .find(|i| i.kind == ItemKind::Imports)
                .map(|i| i.name.as_str()),
            Some("os.path")
        );
    }

    #[test]
    fn relative_import_uses_module_field() {
        // `from .pkg import y`: the module is the relative_import `.pkg`, not the
        // imported symbol `y`.
        let src = "from .pkg import y\ndef main():\n    pass\n";
        let with_imports = extract(src, true);
        assert_eq!(
            with_imports
                .iter()
                .find(|i| i.kind == ItemKind::Imports)
                .map(|i| i.name.as_str()),
            Some(".pkg")
        );
    }

    #[test]
    fn truncated_input_does_not_panic() {
        let _ = extract("def foo():\n", false);
        let _ = extract("class C:\n", false);
        let _ = extract("@deco\n", false);
    }

    #[test]
    fn ranges_are_locator_compatible() {
        let src = "def a():\n    pass\n\nclass B:\n    pass\n";
        for item in &extract(src, false) {
            let loc = format!("[{},{}]", item.start_line, item.end_line);
            crate::locator::Locator::parse(&loc).unwrap();
            for child in &item.children {
                let loc = format!("[{},{}]", child.start_line, child.end_line);
                crate::locator::Locator::parse(&loc).unwrap();
            }
        }
    }
}
