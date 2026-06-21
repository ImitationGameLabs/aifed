//! TypeScript (and TSX) outline extraction via tree-sitter.
//!
//! Two specs share one grammar family: [`TypescriptSpec`] (`.ts`) and
//! [`TsxSpec`] (`.tsx`); [`extract`] picks by extension. Both delegate
//! `classify`/`expand`/`doc_policy` to the shared [`super::ecmascript`] module,
//! which includes the TS-only kinds (interface/enum/abstract-class/type-alias).

use std::path::Path;

use tree_sitter::{Language, Node};

use super::ecmascript;
use super::model::OutlineItem;
use super::spec::{Classified, DocPolicy, Spec};
use super::walker;
use crate::error::Result;

/// Extract a TypeScript/TSX source outline. `.tsx` uses the TSX grammar;
/// everything else (`.ts`) uses the TypeScript grammar. `imports` controls
/// whether `import` items appear.
pub fn extract(source: &str, imports: bool, path: &Path) -> Result<Vec<OutlineItem>> {
    let is_tsx = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("tsx"));
    if is_tsx {
        walker::extract(&TsxSpec, source, imports, path)
    } else {
        walker::extract(&TypescriptSpec, source, imports, path)
    }
}

/// Per-language spec for TypeScript (`.ts`).
pub struct TypescriptSpec;

/// Per-language spec for TSX (`.tsx`): the TypeScript grammar with JSX.
pub struct TsxSpec;

impl Spec for TypescriptSpec {
    fn language(&self) -> Language {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    }

    fn grammar_name(&self) -> &'static str {
        "typescript"
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

impl Spec for TsxSpec {
    fn language(&self) -> Language {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    }

    fn grammar_name(&self) -> &'static str {
        // Both dialects are the TypeScript grammar family; report one name
        // (the registry keys both .ts/.tsx under "typescript").
        "typescript"
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
        super::extract(src, imports, Path::new("test.ts")).unwrap()
    }

    fn extract_tsx(src: &str, imports: bool) -> Vec<OutlineItem> {
        super::extract(src, imports, Path::new("test.tsx")).unwrap()
    }

    #[test]
    fn ts_only_kinds() {
        let src = "interface Box {\n    v: number;\n}\nenum Color {\n    Red,\n}\nabstract class Shape {\n    abstract area(): number;\n}\ntype ID = number;\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "Box").kind, ItemKind::Interface);
        assert_eq!(find(&items, "Color").kind, ItemKind::Enum);
        assert_eq!(find(&items, "Shape").kind, ItemKind::Class);
        assert_eq!(find(&items, "ID").kind, ItemKind::TypeAlias);
    }

    #[test]
    fn shares_js_function_and_class() {
        let src = "function add(a: number, b: number): number {\n    return a + b;\n}\nclass Point {\n    x = 0;\n}\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "add").kind, ItemKind::Function);
        assert_eq!(find(&items, "Point").kind, ItemKind::Class);
    }

    #[test]
    fn tsx_uses_tsx_grammar() {
        // JSX in a .tsx file parses under LANGUAGE_TSX; a `.ts` parse of the
        // same text would error. This confirms the grammar is selected by ext.
        let src = "const El = () => <div />;\nfunction main() {}\n";
        let items = extract_tsx(src, false);
        assert_eq!(find(&items, "El").kind, ItemKind::Function);
        assert_eq!(find(&items, "main").kind, ItemKind::Function);
    }

    #[test]
    fn tsx_uppercase_extension_picks_tsx_grammar() {
        // Case-insensitive: `.TSX` must route to the TSX grammar like `.tsx`.
        let src = "const El = () => <div />;\n";
        let items = super::extract(src, false, Path::new("test.TSX")).unwrap();
        assert_eq!(find(&items, "El").kind, ItemKind::Function);
    }

    #[test]
    fn imports_hidden_by_default_shown_with_flag() {
        let src = "import { h } from 'lib';\nfunction main() {}\n";
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
            Some("lib")
        );
    }

    #[test]
    fn truncated_input_does_not_panic() {
        let _ = extract("interface Box {\n", false);
        let _ = extract("function foo(:\n", false);
        let _ = extract_tsx("const El = () => <\n", false);
    }

    #[test]
    fn ranges_are_locator_compatible() {
        let src = "interface A {}\nfunction b() {}\nclass C {\n    m() {}\n}\n";
        for item in &extract(src, false) {
            let loc = format!("[{},{}]", item.start_line, item.end_line);
            crate::locator::Locator::parse(&loc).unwrap();
        }
    }
}
