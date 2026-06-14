//! Debug helper: print the tree-sitter parse tree with field names.
//! Usage: cargo run --example dump -- 'class A { int i = 1; }'

fn main() {
    let src = std::env::args().nth(1).unwrap_or_else(|| "class A { int i = 1; }".to_string());
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_java::LANGUAGE.into()).unwrap();
    let tree = parser.parse(&src, None).unwrap();
    print_node(tree.root_node(), &src, 0, None);
}

fn print_node(node: tree_sitter::Node, src: &str, depth: usize, field: Option<&str>) {
    let indent = "  ".repeat(depth);
    let fieldpfx = field.map(|f| format!("{f}: ")).unwrap_or_default();
    let text = if node.child_count() == 0 {
        format!("  {:?}", &src[node.byte_range()])
    } else {
        String::new()
    };
    let sp = node.start_position();
    let ep = node.end_position();
    println!(
        "{indent}{fieldpfx}{}{text}   [{},{} - {},{}]",
        node.kind(),
        sp.row, sp.column, ep.row, ep.column
    );
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let fname = cursor.field_name();
            print_node(cursor.node(), src, depth + 1, fname);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
