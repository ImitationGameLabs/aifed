//! C++ outline extraction via tree-sitter.
//!
//! [`CppSpec`] extends the C spec: it reuses [`super::c`] for the shared kinds
//! (functions, structs/enums/unions, typedefs, macros, includes, prototypes) and
//! the preprocessor/declaration piercing, then adds the C++-only kinds —
//! classes, namespaces, `using` aliases, and concepts. Classes, structs, and
//! namespaces recurse into their bodies so nested methods/declarations surface
//! as children.

use std::path::Path;

use tree_sitter::{Language, Node};

use super::c;
use super::helpers::{declarator_name, field_text};
use super::model::{ItemKind, OutlineItem};
use super::spec::{Classified, DocPolicy, Spec};
use super::walker;
use crate::error::Result;

/// Extract a C++ source outline. `imports` controls whether `#include` items
/// appear.
pub fn extract(source: &str, imports: bool, path: &Path) -> Result<Vec<OutlineItem>> {
    walker::extract(&CppSpec, source, imports, path)
}

/// Per-language spec driving the generic walker for C++.
pub struct CppSpec;

/// Doc attribution for C++: same as C — a contiguous preceding comment block is
/// the doc, with a blank-line break.
const CPP_DOC_POLICY: DocPolicy = DocPolicy {
    attribute_kinds: &[],
    decorator_kinds: &[],
    attach_extras: true,
    doc_prefixes: &[],
    expand_backward: true,
    blank_line_breaks: true,
};

impl Spec for CppSpec {
    fn language(&self) -> Language {
        tree_sitter_cpp::LANGUAGE.into()
    }

    fn grammar_name(&self) -> &'static str {
        "cpp"
    }

    fn expand<'a>(&self, node: Node<'a>) -> Vec<Node<'a>> {
        c::pierce(node)
    }

    fn classify<'a>(&self, node: Node<'a>, source: &str) -> Option<Classified<'a>> {
        classify(node, source)
    }

    fn doc_policy(&self) -> &'static DocPolicy {
        &CPP_DOC_POLICY
    }
}

/// Classify a C++ node: handle the C++-only container/type kinds, then fall back
/// to the shared C [`c::classify`] for everything else.
fn classify<'a>(node: Node<'a>, source: &str) -> Option<Classified<'a>> {
    let (kind, name, body) = match node.kind() {
        "class_specifier" => (
            ItemKind::Class,
            class_name(node, source),
            node.child_by_field_name("body"),
        ),
        // C++ structs can hold methods like classes; recurse into the body.
        "struct_specifier" => (
            ItemKind::Struct,
            c::tag_name(node, source),
            node.child_by_field_name("body"),
        ),
        "namespace_definition" => (
            ItemKind::Module,
            namespace_name(node, source),
            node.child_by_field_name("body"),
        ),
        "alias_declaration" => (ItemKind::TypeAlias, field_text(node, "name", source), None),
        "concept_definition" => (ItemKind::Trait, field_text(node, "name", source), None),
        "field_declaration" => {
            // A class-member method declaration (`void draw();`); its declarator
            // holds a function_declarator. Data members (`int x_;`) do not.
            let decl = node.child_by_field_name("declarator")?;
            if c::is_function_decl(decl) {
                (ItemKind::Function, declarator_name(decl, source), None)
            } else {
                return None;
            }
        }
        _ => return c::classify(node, source),
    };
    if name.is_empty() {
        return None;
    }
    Some(Classified { kind, name, body, detail: None, imports_gated: false })
}

/// Name of a `class_specifier`: the `name` field is polymorphic
/// (`type_identifier`/`qualified_identifier`/`template_type`); take the whole
/// text (e.g. `NS::Foo`, `Foo<T>`) as the display name.
fn class_name(node: Node<'_>, source: &str) -> String {
    field_text(node, "name", source)
}

/// Name of a `namespace_definition` (`namespace_identifier` or a nested
/// `A::B`); anonymous namespaces get a placeholder so their body still recurses.
fn namespace_name(node: Node<'_>, source: &str) -> String {
    let name = field_text(node, "name", source);
    if name.is_empty() { "<anonymous>".to_string() } else { name }
}

#[cfg(test)]
mod tests {
    use crate::outline::model::{ItemKind, OutlineItem};
    use crate::outline::test_support::find;
    use std::path::Path;

    fn extract(src: &str, imports: bool) -> Vec<OutlineItem> {
        super::extract(src, imports, Path::new("test.cpp")).unwrap()
    }

    #[test]
    fn class_recurses_into_methods() {
        let src = "class Foo {\npublic:\n    void greet() {}\n    int value() { return 0; }\n};\n";
        let items = extract(src, false);
        let foo = find(&items, "Foo");
        assert_eq!(foo.kind, ItemKind::Class);
        assert_eq!(
            foo.children
                .iter()
                .find(|m| m.name == "greet")
                .map(|m| m.kind),
            Some(ItemKind::Function)
        );
        assert_eq!(
            foo.children
                .iter()
                .find(|m| m.name == "value")
                .map(|m| m.kind),
            Some(ItemKind::Function)
        );
    }

    #[test]
    fn struct_recurses_into_methods() {
        let src = "struct S {\n    void m() {}\n};\n";
        let items = extract(src, false);
        let s = find(&items, "S");
        assert_eq!(s.kind, ItemKind::Struct);
        assert_eq!(
            s.children.iter().find(|m| m.name == "m").map(|m| m.kind),
            Some(ItemKind::Function)
        );
    }

    #[test]
    fn namespace_recurses() {
        let src = "namespace ns {\n    void f() {}\n}\n";
        let items = extract(src, false);
        let ns = find(&items, "ns");
        assert_eq!(ns.kind, ItemKind::Module);
        assert_eq!(
            ns.children.iter().find(|c| c.name == "f").map(|c| c.kind),
            Some(ItemKind::Function)
        );
    }

    #[test]
    fn nested_namespace() {
        let src = "namespace a {\n    namespace b {\n        void g() {}\n    }\n}\n";
        let items = extract(src, false);
        let a = find(&items, "a");
        let b = a
            .children
            .iter()
            .find(|c| c.name == "b")
            .expect("nested namespace b");
        assert_eq!(b.kind, ItemKind::Module);
        assert_eq!(
            b.children.iter().find(|c| c.name == "g").map(|c| c.kind),
            Some(ItemKind::Function)
        );
    }

    #[test]
    fn anonymous_namespace_gets_placeholder() {
        let src = "namespace {\n    void h() {}\n}\n";
        let items = extract(src, false);
        let anon = find(&items, "<anonymous>");
        assert_eq!(anon.kind, ItemKind::Module);
        assert_eq!(
            anon.children.iter().find(|c| c.name == "h").map(|c| c.kind),
            Some(ItemKind::Function)
        );
    }

    #[test]
    fn using_alias_and_concept() {
        let src = "using MyInt = int;\nconcept C = true;\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "MyInt").kind, ItemKind::TypeAlias);
        assert_eq!(find(&items, "C").kind, ItemKind::Trait);
    }

    #[test]
    fn reuses_c_function_and_typedef() {
        let src = "int *foo(void) {\n    return 0;\n}\ntypedef int Handle;\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "foo").kind, ItemKind::Function);
        assert_eq!(find(&items, "Handle").kind, ItemKind::TypeAlias);
    }

    #[test]
    fn templated_declarations_surface() {
        // template_declaration wraps the definition; it must be pierced.
        let src = "template <class T> T identity(T x) { return x; }\ntemplate <class T> class Box { public: T get() { return t_; } private: T t_; };\ntemplate <class T> concept Addable = true;\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "identity").kind, ItemKind::Function);
        let box_item = find(&items, "Box");
        assert_eq!(box_item.kind, ItemKind::Class);
        assert_eq!(
            box_item
                .children
                .iter()
                .find(|m| m.name == "get")
                .map(|m| m.kind),
            Some(ItemKind::Function)
        );
        assert_eq!(find(&items, "Addable").kind, ItemKind::Trait);
    }

    #[test]
    fn class_method_declarations_surface() {
        // A header-style class: method DECLARATIONS (`void draw();`) are
        // field_declarations with a function_declarator; data members are skipped.
        let src = "class Widget {\npublic:\n    void draw();\n    int area() const;\nprivate:\n    int x_;\n};\n";
        let items = extract(src, false);
        let w = find(&items, "Widget");
        assert_eq!(
            w.children.iter().find(|m| m.name == "draw").map(|m| m.kind),
            Some(ItemKind::Function)
        );
        assert_eq!(
            w.children.iter().find(|m| m.name == "area").map(|m| m.kind),
            Some(ItemKind::Function)
        );
        assert!(
            w.children.iter().all(|m| m.name != "x_"),
            "{:?}",
            w.children
        );
    }

    #[test]
    fn out_of_line_method_name_is_unqualified() {
        // `void Foo::bar()` resolves to `bar`, not the qualifier `Foo`.
        let src = "void Foo::bar() {}\nvoid Foo::baz(int x) {}\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "bar").kind, ItemKind::Function);
        assert_eq!(find(&items, "baz").kind, ItemKind::Function);
        assert!(
            items.iter().all(|i| i.name != "Foo"),
            "qualifier leaked as a name"
        );
    }

    #[test]
    fn truncated_input_does_not_panic() {
        let _ = extract("class Foo {\n", false);
        let _ = extract("namespace ns {\n", false);
        let _ = extract("template <typename T>\n", false);
    }

    #[test]
    fn ranges_are_locator_compatible() {
        let src = "class A {\n    void m() {}\n};\nvoid b() {}\n";
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
