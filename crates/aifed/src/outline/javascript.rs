//! JavaScript outline extraction via tree-sitter.
//!
//! [`JavascriptSpec`] is a thin wrapper: it supplies the JavaScript grammar and
//! name, then delegates [`Spec::expand`]/[`Spec::classify`]/[`Spec::doc_policy`]
//! to the shared [`super::ecmascript`] module (reused by TypeScript).

use std::path::Path;

use tree_sitter::{Language, Node};

use super::ecmascript;
use super::model::OutlineItem;
use super::spec::{Classified, DocPolicy, Spec};
use super::walker;
use crate::error::Result;

/// Extract a JavaScript source outline. `imports` controls whether `import`
/// items appear.
pub fn extract(source: &str, imports: bool, path: &Path) -> Result<Vec<OutlineItem>> {
    walker::extract(&JavascriptSpec, source, imports, path)
}

/// Per-language spec driving the generic walker for JavaScript.
pub struct JavascriptSpec;

impl Spec for JavascriptSpec {
    fn language(&self) -> Language {
        tree_sitter_javascript::LANGUAGE.into()
    }

    fn grammar_name(&self) -> &'static str {
        "javascript"
    }

    fn expand<'a>(&self, node: Node<'a>) -> Vec<Node<'a>> {
        ecmascript::expand(node)
    }

    fn classify<'a>(&self, node: Node<'a>, source: &str) -> Option<Classified<'a>> {
        ecmascript::classify(node, source)
    }

    fn doc_policy(&self) -> &'static DocPolicy {
        &ecmascript::ECMASCRIPT_DOC_POLICY
    }
}

#[cfg(test)]
mod tests {
    use crate::outline::model::{ItemKind, OutlineItem};
    use crate::outline::test_support::find;
    use std::path::Path;

    fn extract(src: &str, imports: bool) -> Vec<OutlineItem> {
        super::extract(src, imports, Path::new("test.js")).unwrap()
    }

    #[test]
    fn function_generator_and_class_kinds() {
        let src = "function add(a, b) {\n    return a + b;\n}\n\
                   function* gen() {\n    yield 1;\n\
                   }\n\
                   class Point {\n    constructor() {}\n}\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "add").kind, ItemKind::Function);
        assert_eq!(find(&items, "gen").kind, ItemKind::Function);
        assert_eq!(find(&items, "Point").kind, ItemKind::Class);
    }

    #[test]
    fn class_method_is_child() {
        let src = "class C {\n    greet() {\n        return 1;\n    }\n}\n";
        let items = extract(src, false);
        let c = find(&items, "C");
        assert_eq!(
            c.children
                .iter()
                .find(|m| m.name == "greet")
                .map(|m| m.kind),
            Some(ItemKind::Function)
        );
    }

    #[test]
    fn named_arrow_function_is_function() {
        let src = "const f = () => {\n    return 1;\n};\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "f").kind, ItemKind::Function);
    }

    #[test]
    fn plain_binding_is_variable() {
        let src = "const N = 1;\nlet x;\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "N").kind, ItemKind::Variable);
        assert_eq!(find(&items, "x").kind, ItemKind::Variable);
    }

    #[test]
    fn export_unwraps_to_declaration() {
        let src = "export function foo() {}\nexport const K = 2;\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "foo").kind, ItemKind::Function);
        assert_eq!(find(&items, "K").kind, ItemKind::Variable);
    }

    #[test]
    fn imports_hidden_by_default_shown_with_flag() {
        let src = "import foo from 'mod';\nfunction main() {}\n";
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
            Some("mod")
        );
    }

    #[test]
    fn truncated_input_does_not_panic() {
        let _ = extract("function foo() {\n", false);
        let _ = extract("class C {\n", false);
        let _ = extract("const x =\n", false);
    }

    #[test]
    fn ranges_are_locator_compatible() {
        let src = "function a() {}\nclass B {\n    m() {}\n}\n";
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
