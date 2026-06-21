//! C outline extraction via tree-sitter.
//!
//! Drives the generic [`super::walker`] via [`CSpec`]. C declarations are
//! syntactically top-level (no nesting), so every item is a leaf (`body: None`).
//! Three C-isms need attention: names are buried in declarator chains (extracted
//! via [`super::helpers::declarator_name`]); preprocessor blocks
//! (`#if`/`#ifdef`/`#else`/`#elif`) wrap real definitions, so [`Spec::expand`]
//! pierces them; and a top-level `declaration` may be a struct/enum/union
//! definition (pierced to its specifier), a function prototype (kept), or a
//! plain variable (skipped).
//!
//! [`classify`], [`pierce`], and [`tag_name`] are `pub(super)` so the C++ spec
//! (`super::cpp`) can reuse the shared C kinds and piercing.

use std::path::Path;

use tree_sitter::{Language, Node};

use super::helpers::{declarator_name, field_text, named_children};
use super::model::{ItemKind, OutlineItem};
use super::spec::{Classified, DocPolicy, Spec};
use super::walker;
use crate::error::Result;

/// Extract a C source outline. `imports` controls whether `#include` items
/// appear.
pub fn extract(source: &str, imports: bool, path: &Path) -> Result<Vec<OutlineItem>> {
    walker::extract(&CSpec, source, imports, path)
}

/// Per-language spec driving the generic walker for C.
pub struct CSpec;

/// Doc attribution for C: a contiguous preceding `//`/`/* */` block is the doc
/// (`doc_prefixes: &[]` attaches every comment), with a blank-line break. C has
/// no formal doc convention, so any directly-preceding comment qualifies.
const C_DOC_POLICY: DocPolicy = DocPolicy {
    attribute_kinds: &[],
    decorator_kinds: &[],
    attach_extras: true,
    doc_prefixes: &[],
    expand_backward: true,
    blank_line_breaks: true,
};

impl Spec for CSpec {
    fn language(&self) -> Language {
        tree_sitter_c::LANGUAGE.into()
    }

    fn grammar_name(&self) -> &'static str {
        "c"
    }

    fn expand<'a>(&self, node: Node<'a>) -> Vec<Node<'a>> {
        pierce(node)
    }

    fn classify<'a>(&self, node: Node<'a>, source: &str) -> Option<Classified<'a>> {
        classify(node, source)
    }

    fn doc_policy(&self) -> &'static DocPolicy {
        &C_DOC_POLICY
    }
}

/// Classify one (already-pierced) C node, or `None` to skip it. Shared with C++.
pub(super) fn classify<'a>(node: Node<'a>, source: &str) -> Option<Classified<'a>> {
    let (kind, name) = match node.kind() {
        // The name lives in the declarator chain (`int *foo(void)` -> foo).
        "function_definition" => {
            let name = node
                .child_by_field_name("declarator")
                .map(|d| declarator_name(d, source))
                .unwrap_or_default();
            (ItemKind::Function, name)
        }
        "struct_specifier" => (ItemKind::Struct, tag_name(node, source)),
        "enum_specifier" => (ItemKind::Enum, tag_name(node, source)),
        "union_specifier" => (ItemKind::Union, tag_name(node, source)),
        // typedef: name in the (first) declarator chain, terminal type_identifier.
        // A multi-name typedef (`typedef int a, b;`) surfaces only `a`.
        "type_definition" => {
            let name = node
                .child_by_field_name("declarator")
                .map(|d| declarator_name(d, source))
                .unwrap_or_default();
            (ItemKind::TypeAlias, name)
        }
        "preproc_def" | "preproc_function_def" => {
            (ItemKind::Macro, field_text(node, "name", source))
        }
        "preproc_include" => (ItemKind::Imports, include_path(node, source)),
        // A function prototype (`int foo(void);`): its declarator chain holds
        // a function_declarator. Plain variable declarations do not and fall
        // through to `None`. A declaration wrapping a struct/enum/union
        // definition is pierced to its specifier by `pierce` (never reaching
        // here).
        "declaration" => {
            let decl = node.child_by_field_name("declarator")?;
            if is_function_decl(decl) {
                (ItemKind::Function, declarator_name(decl, source))
            } else {
                return None;
            }
        }
        _ => return None,
    };
    if name.is_empty() {
        return None;
    }
    let imports_gated = matches!(kind, ItemKind::Imports);
    Some(Classified { kind, name, body: None, detail: None, imports_gated })
}

/// Fan preprocessor blocks, C++ templates, and type-defining declarations into the
/// carry a name. `preproc_*` blocks recurse (an `#if` can contain `#else`); a
/// `declaration` whose `type` is a struct/enum/union/class definition yields that
/// specifier, while any other declaration (a prototype or variable) is left for
/// [`classify`] (which keeps prototypes and drops plain variables). Shared with
/// C++.
pub(super) fn pierce(node: Node<'_>) -> Vec<Node<'_>> {
    match node.kind() {
        "preproc_if"
        | "preproc_ifdef"
        | "preproc_else"
        | "preproc_elif"
        | "preproc_elifdef"
        | "template_declaration" => named_children(node).into_iter().flat_map(pierce).collect(),
        "declaration" => match node.child_by_field_name("type") {
            Some(t)
                if matches!(
                    t.kind(),
                    "struct_specifier" | "enum_specifier" | "union_specifier" | "class_specifier"
                ) =>
            {
                vec![t]
            }
            _ => vec![node],
        },
        _ => vec![node],
    }
}

/// Whether a declarator chain contains a `function_declarator` (it names a
/// function/prototype) rather than a plain variable. Depth-capped like
/// [`declarator_name`].
pub(super) fn is_function_decl(mut decl: Node<'_>) -> bool {
    for _ in 0..32 {
        match decl.kind() {
            "function_declarator" => return true,
            "identifier" | "type_identifier" => return false,
            _ => match decl.child_by_field_name("declarator") {
                Some(inner) => decl = inner,
                None => return false,
            },
        }
    }
    false
}

/// Tag name of a `struct`/`enum`/`union` specifier; `"<anonymous>"` when the
/// tag is absent (e.g. `struct { int x; } instance;`). Two anonymous tags in one
/// file collide on this placeholder. Shared with C++.
pub(super) fn tag_name(node: Node<'_>, source: &str) -> String {
    let name = field_text(node, "name", source);
    if name.is_empty() { "<anonymous>".to_string() } else { name }
}

/// Path of a `#include`, with surrounding quotes or angle brackets stripped.
fn include_path(node: Node<'_>, source: &str) -> String {
    field_text(node, "path", source)
        .trim_matches(|ch| ch == '"' || ch == '<' || ch == '>')
        .to_string()
}

#[cfg(test)]
mod tests {
    use crate::outline::model::{ItemKind, OutlineItem};
    use crate::outline::test_support::find;
    use std::path::Path;

    fn extract(src: &str, imports: bool) -> Vec<OutlineItem> {
        super::extract(src, imports, Path::new("test.c")).unwrap()
    }

    #[test]
    fn function_name_through_declarator_chain() {
        let src = "int *foo(void) {\n    return 0;\n}\nvoid bar(int x) {\n}\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "foo").kind, ItemKind::Function);
        assert_eq!(find(&items, "bar").kind, ItemKind::Function);
    }

    #[test]
    fn function_prototypes_surface_as_functions() {
        // A header of prototypes: each is a `declaration` with a function_declarator.
        let src = "int open(const char *);\nvoid close(int);\nint g_counter;\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "open").kind, ItemKind::Function);
        assert_eq!(find(&items, "close").kind, ItemKind::Function);
        // Plain variable declarations are skipped, not mistaken for functions.
        assert!(items.iter().all(|i| i.name != "g_counter"), "{items:?}");
    }

    #[test]
    fn struct_enum_union_kinds() {
        let src = "struct Point {\n    int x;\n};\nenum Color {\n    RED,\n};\nunion Tag {\n    int i;\n};\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "Point").kind, ItemKind::Struct);
        assert_eq!(find(&items, "Color").kind, ItemKind::Enum);
        assert_eq!(find(&items, "Tag").kind, ItemKind::Union);
    }

    #[test]
    fn anonymous_struct_gets_placeholder_name() {
        let src = "struct {\n    int x;\n} g;\n";
        assert_eq!(
            find(&extract(src, false), "<anonymous>").kind,
            ItemKind::Struct
        );
    }

    #[test]
    fn typedef_name() {
        let src = "typedef int MyInt;\ntypedef struct {\n    int x;\n} Handle;\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "MyInt").kind, ItemKind::TypeAlias);
        assert_eq!(find(&items, "Handle").kind, ItemKind::TypeAlias);
    }

    #[test]
    fn object_and_function_macros() {
        let src = "#define MAX 10\n#define ADD(a, b) ((a) + (b))\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "MAX").kind, ItemKind::Macro);
        assert_eq!(find(&items, "ADD").kind, ItemKind::Macro);
    }

    #[test]
    fn includes_hidden_by_default_shown_with_flag() {
        let src = "#include <stdio.h>\n#include \"local.h\"\nvoid main(void) {}\n";
        assert!(
            extract(src, false)
                .iter()
                .all(|i| i.kind != ItemKind::Imports),
            "#include hidden without --imports"
        );
        let with_imports = extract(src, true);
        let names: Vec<&str> = with_imports
            .iter()
            .filter(|i| i.kind == ItemKind::Imports)
            .map(|i| i.name.as_str())
            .collect();
        assert!(names.contains(&"stdio.h"), "{names:?}");
        assert!(names.contains(&"local.h"), "{names:?}");
    }

    #[test]
    fn preproc_if_pierces_to_inner_definitions() {
        let src = "#ifdef DEBUG\nvoid debug(void) {}\n#endif\nvoid release(void) {}\n";
        let items = extract(src, false);
        assert_eq!(find(&items, "debug").kind, ItemKind::Function);
        assert_eq!(find(&items, "release").kind, ItemKind::Function);
    }

    #[test]
    fn truncated_input_does_not_panic() {
        let _ = extract("int foo() {\n", false);
        let _ = extract("struct {\n", false);
        let _ = extract("#ifdef X\n", false);
    }

    #[test]
    fn ranges_are_locator_compatible() {
        let src = "void a(void) {}\nstruct B {\n    int x;\n};\n";
        for item in &extract(src, false) {
            let loc = format!("[{},{}]", item.start_line, item.end_line);
            crate::locator::Locator::parse(&loc).unwrap();
        }
    }
}
