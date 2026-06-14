//! tree-sitter front end + adapter to the typed arena AST.
//!
//! Mirrors `PartParser.createCompilationUnit`: try to parse the source as-is;
//! on a parse error, retry wrapped in a class, then wrapped in a method.

use crate::ast::{Arena, NodeId};

/// Parse Java source into the arena, returning the root `CompilationUnit` id.
///
/// Mirrors `PartParser.createCompilationUnit(String)` followed by building the
/// typed AST. Returns `None` if all three parse attempts fail.
pub fn create_compilation_unit(java: &str) -> Option<(Arena, NodeId)> {
    let candidates = [
        java.to_string(),
        encapsulate_in_class(java),
        encapsulate_in_method(java),
    ];
    for (tier, src) in candidates.iter().enumerate() {
        if let Some(tree) = try_parse(src) {
            if !valid_for_tier(tier, tree.root_node(), src) {
                continue;
            }
            let mut arena = Arena::new();
            let root = crate::adapter::build(&mut arena, src, &tree);
            arena.root = Some(root);
            return Some((arena, root));
        }
    }
    None
}

/// tree-sitter accepts many fragments JavaParser rejects (keywords as
/// identifiers, statements at top level, ...). Replicate JavaParser's stricter
/// acceptance so the PartParser wrapping fallback picks the same tier.
fn valid_for_tier(tier: usize, root: tree_sitter::Node, src: &str) -> bool {
    if tier == 0 {
        // A real CompilationUnit: only declaration-level children.
        let mut cur = root.walk();
        for c in root.named_children(&mut cur) {
            if !matches!(
                c.kind(),
                "package_declaration"
                    | "import_declaration"
                    | "class_declaration"
                    | "interface_declaration"
                    | "enum_declaration"
                    | "annotation_type_declaration"
                    | "line_comment"
                    | "block_comment"
            ) {
                return false;
            }
        }
        true
    } else {
        // Wrapped forms: reject if tree-sitter recovered a reserved word as a
        // type name (a sign that the fragment isn't a valid member/declaration).
        !contains_keyword_type_identifier(root, src)
    }
}

fn contains_keyword_type_identifier(node: tree_sitter::Node, src: &str) -> bool {
    let bytes = src.as_bytes();
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        if n.kind() == "type_identifier" {
            let text = std::str::from_utf8(&bytes[n.byte_range()]).unwrap_or("");
            if RESERVED.contains(&text) {
                return true;
            }
        }
        let mut cur = n.walk();
        for c in n.children(&mut cur) {
            stack.push(c);
        }
    }
    false
}

/// Java reserved words that cannot be a type name; tree-sitter only emits these
/// as `type_identifier` when recovering from a non-declaration fragment.
const RESERVED: &[&str] = &[
    "abstract", "assert", "boolean", "break", "byte", "case", "catch", "char", "class", "const",
    "continue", "default", "do", "double", "else", "enum", "extends", "final", "finally", "float",
    "for", "goto", "if", "implements", "import", "instanceof", "int", "interface", "long", "native",
    "new", "package", "private", "protected", "public", "return", "short", "static", "strictfp",
    "super", "switch", "synchronized", "this", "throw", "throws", "transient", "try", "void",
    "volatile", "while", "true", "false", "null",
];

fn encapsulate_in_class(test_string: &str) -> String {
    format!("class A {{ {test_string};  }}")
}

fn encapsulate_in_method(test_string: &str) -> String {
    format!("class A {{ void m() {{ {test_string}; }} }}")
}

/// Parse with tree-sitter; `None` if the tree has any error.
fn try_parse(src: &str) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .expect("load tree-sitter-java");
    let tree = parser.parse(src, None)?;
    if tree.root_node().has_error() {
        None
    } else {
        Some(tree)
    }
}
