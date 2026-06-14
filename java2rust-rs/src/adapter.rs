//! tree-sitter CST -> typed arena AST.
//!
//! The single place tree-sitter's concrete syntax tree is translated into the
//! JavaParser-shaped typed AST the rest of the converter is written against.

use tree_sitter::Node as TsNode;

use crate::ast::{Arena, BinaryOp, Node, NodeId, Pos, PrimitiveKind, UnaryOp};
use crate::modifiers;

pub fn build(arena: &mut Arena, src: &str, tree: &tree_sitter::Tree) -> NodeId {
    let mut b = Builder {
        arena,
        src: src.as_bytes(),
    };
    let root = b.compilation_unit(tree.root_node());
    let comments = b.collect_comments(tree.root_node());
    set_parents(arena, root);
    attribute_comments(arena, root, comments);
    root
}

/// Attach comments to nodes (`node.comment`) and orphan-comment lists, mirroring
/// JavaParser's comment attribution.
fn attribute_comments(arena: &mut Arena, root: NodeId, comments: Vec<NodeId>) {
    insert_comments(arena, root, comments, root);
}

fn pos_le(a: Pos, b: Pos) -> bool {
    (a.line, a.column) <= (b.line, b.column)
}
fn pos_lt(a: Pos, b: Pos) -> bool {
    (a.line, a.column) < (b.line, b.column)
}

fn contained(arena: &Arena, child: NodeId, c: NodeId) -> bool {
    pos_le(arena.begin(child), arena.begin(c)) && pos_le(arena.end(c), arena.end(child))
}

fn insert_comments(arena: &mut Arena, node: NodeId, mut comments: Vec<NodeId>, root: NodeId) {
    if comments.is_empty() {
        return;
    }
    // AST children only (no comments attached yet), sorted by begin position.
    let mut children = arena.children(node);
    children.sort_by_key(|&c| {
        let p = arena.begin(c);
        (p.line, p.column)
    });

    // 1. Distribute comments contained within a child down into that child.
    for &child in &children {
        let inside: Vec<NodeId> = comments
            .iter()
            .copied()
            .filter(|&c| contained(arena, child, c))
            .collect();
        if !inside.is_empty() {
            comments.retain(|c| !inside.contains(c));
            insert_comments(arena, child, inside, root);
        }
    }

    // 2. Attribute the closest preceding comment to each child as its comment.
    let mut used = vec![false; comments.len()];
    for &child in &children {
        if arena.comment(child).is_some() {
            continue;
        }
        let cb = arena.begin(child);
        let mut last: Option<usize> = None;
        for (i, &c) in comments.iter().enumerate() {
            if !used[i] && pos_lt(arena.begin(c), cb) {
                last = Some(i);
            }
        }
        if let Some(i) = last {
            let c = comments[i];
            arena.data_mut(child).comment = Some(c);
            arena.data_mut(c).parent = Some(child);
            used[i] = true;
        }
    }

    // 3. Only the root CompilationUnit claims a leftover leading comment as its
    //    own (e.g. a file license header). For any other node the own-comment is
    //    set by its parent; internal leftovers stay orphans.
    if node == root && arena.comment(node).is_none() {
        let first_begin = children.first().map(|&c| arena.begin(c));
        let mut chosen: Option<usize> = None;
        for (i, &c) in comments.iter().enumerate() {
            if used[i] {
                continue;
            }
            let before = match first_begin {
                Some(fb) => pos_lt(arena.begin(c), fb),
                None => true,
            };
            if before {
                chosen = Some(i);
            }
        }
        if let Some(i) = chosen {
            let c = comments[i];
            arena.data_mut(node).comment = Some(c);
            arena.data_mut(c).parent = Some(node);
            used[i] = true;
        }
    }

    // 4. Everything else is an orphan comment of this node.
    for (i, &c) in comments.iter().enumerate() {
        if !used[i] {
            arena.data_mut(node).orphan_comments.push(c);
            arena.data_mut(c).parent = Some(node);
        }
    }
}

/// Set `parent` on every node by walking `children()` from the root.
fn set_parents(arena: &mut Arena, root: NodeId) {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        for child in arena.children(id) {
            arena.data_mut(child).parent = Some(id);
            stack.push(child);
        }
    }
}

struct Builder<'a> {
    arena: &'a mut Arena,
    src: &'a [u8],
}

impl<'a> Builder<'a> {
    fn text(&self, n: TsNode) -> String {
        std::str::from_utf8(&self.src[n.byte_range()])
            .unwrap()
            .to_string()
    }

    /// JavaParser positions: 1-based line, 1-based inclusive column.
    fn positions(&self, n: TsNode) -> (Pos, Pos) {
        let s = n.start_position();
        let e = n.end_position();
        (
            Pos {
                line: s.row as i32 + 1,
                column: s.column as i32 + 1,
            },
            Pos {
                line: e.row as i32 + 1,
                column: e.column as i32,
            },
        )
    }

    fn alloc(&mut self, kind: Node, n: TsNode) -> NodeId {
        let (b, e) = self.positions(n);
        self.arena.alloc(kind, b, e)
    }

    // ---- child navigation helpers ----

    fn field(&self, n: TsNode<'a>, name: &str) -> Option<TsNode<'a>> {
        n.child_by_field_name(name)
    }

    fn fields(&self, n: TsNode<'a>, name: &str) -> Vec<TsNode<'a>> {
        let mut cur = n.walk();
        let out: Vec<TsNode> = n.children_by_field_name(name, &mut cur).collect();
        out
    }

    fn named_children(&self, n: TsNode<'a>) -> Vec<TsNode<'a>> {
        let mut cur = n.walk();
        let out: Vec<TsNode> = n.named_children(&mut cur).collect();
        out
    }

    fn all_children(&self, n: TsNode<'a>) -> Vec<TsNode<'a>> {
        let mut cur = n.walk();
        let out: Vec<TsNode> = n.children(&mut cur).collect();
        out
    }

    fn unsupported(&self, what: &str, n: TsNode) -> ! {
        panic!("adapter: unsupported {what} `{}`", n.kind());
    }

    /// Walk the whole tree-sitter tree, allocating an AST comment node for each
    /// comment, returned in source order.
    fn collect_comments(&mut self, root: TsNode<'a>) -> Vec<NodeId> {
        let mut found: Vec<TsNode> = Vec::new();
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            if matches!(n.kind(), "line_comment" | "block_comment") {
                found.push(n);
            }
            let mut cur = n.walk();
            for c in n.children(&mut cur) {
                stack.push(c);
            }
        }
        found.sort_by_key(|n| {
            let p = n.start_position();
            (p.row, p.column)
        });
        found
            .into_iter()
            .map(|n| {
                let raw = self.text(n);
                let node = if n.kind() == "line_comment" {
                    Node::LineComment {
                        content: raw.strip_prefix("//").unwrap_or(&raw).to_string(),
                    }
                } else if raw.starts_with("/**") && raw.len() >= 5 {
                    let inner = &raw[3..raw.len() - 2];
                    Node::JavadocComment {
                        content: inner.to_string(),
                    }
                } else {
                    let inner = if raw.len() >= 4 {
                        &raw[2..raw.len() - 2]
                    } else {
                        ""
                    };
                    Node::BlockComment {
                        content: inner.to_string(),
                    }
                };
                self.alloc(node, n)
            })
            .collect()
    }

    // ---- compilation unit ----

    fn compilation_unit(&mut self, program: TsNode<'a>) -> NodeId {
        let mut package = None;
        let mut imports = Vec::new();
        let mut types = Vec::new();
        for c in self.named_children(program) {
            match c.kind() {
                "package_declaration" => package = Some(self.package_declaration(c)),
                "import_declaration" => imports.push(self.import_declaration(c)),
                "line_comment" | "block_comment" => {} // attached separately later
                _ => types.push(self.type_declaration(c)),
            }
        }
        self.alloc(
            Node::CompilationUnit {
                package,
                imports,
                types,
            },
            program,
        )
    }

    fn package_declaration(&mut self, n: TsNode<'a>) -> NodeId {
        // last named child is the name (scoped_identifier / identifier)
        let name_node = self.named_children(n).into_iter().last().unwrap();
        let name = self.name_expr_of(name_node);
        self.alloc(Node::PackageDeclaration { name }, n)
    }

    fn import_declaration(&mut self, n: TsNode<'a>) -> NodeId {
        let is_static = self.all_children(n).iter().any(|c| c.kind() == "static");
        let is_asterisk = self.all_children(n).iter().any(|c| c.kind() == "asterisk");
        let name_node = self
            .named_children(n)
            .into_iter()
            .find(|c| matches!(c.kind(), "identifier" | "scoped_identifier"))
            .unwrap();
        let name = self.name_expr_of(name_node);
        self.alloc(
            Node::ImportDeclaration {
                name,
                is_static,
                is_asterisk,
            },
            n,
        )
    }

    /// Build a NameExpr / QualifiedNameExpr from an identifier or scoped_identifier.
    fn name_expr_of(&mut self, n: TsNode<'a>) -> NodeId {
        match n.kind() {
            "scoped_identifier" => {
                let scope = self.field(n, "scope").unwrap();
                let name = self.text(self.field(n, "name").unwrap());
                let qualifier = self.name_expr_of(scope);
                self.alloc(Node::QualifiedNameExpr { qualifier, name }, n)
            }
            _ => {
                let name = self.text(n);
                self.alloc(Node::NameExpr { name }, n)
            }
        }
    }

    // ---- type declarations ----

    fn type_declaration(&mut self, n: TsNode<'a>) -> NodeId {
        match n.kind() {
            "class_declaration" => self.class_declaration(n, false),
            "interface_declaration" => self.class_declaration(n, true),
            "enum_declaration" => self.enum_declaration(n),
            ";" => self.alloc(Node::EmptyTypeDeclaration, n),
            _ => self.unsupported("type declaration", n),
        }
    }

    fn class_declaration(&mut self, n: TsNode<'a>, is_interface: bool) -> NodeId {
        let (modifiers, _ann) = self.parse_modifiers(n);
        let name = self.text(self.field(n, "name").unwrap());
        let type_parameters = match self.field(n, "type_parameters") {
            Some(tp) => self.type_parameters(tp),
            None => Vec::new(),
        };
        let mut extends = Vec::new();
        let mut implements = Vec::new();
        for c in self.all_children(n) {
            match c.kind() {
                "superclass" => {
                    for t in self.named_children(c) {
                        extends.push(self.typ(t));
                    }
                }
                "extends_interfaces" => {
                    // interface: `extends` type_list
                    for tl in self.named_children(c) {
                        for t in self.named_children(tl) {
                            extends.push(self.typ(t));
                        }
                    }
                }
                "super_interfaces" => {
                    for tl in self.named_children(c) {
                        for t in self.named_children(tl) {
                            implements.push(self.typ(t));
                        }
                    }
                }
                _ => {}
            }
        }
        let body = self.field(n, "body").unwrap();
        let members = self.members_of(body);
        self.alloc(
            Node::ClassOrInterfaceDeclaration {
                modifiers,
                is_interface,
                name,
                type_parameters,
                extends,
                implements,
                members,
            },
            n,
        )
    }

    fn enum_declaration(&mut self, n: TsNode<'a>) -> NodeId {
        let (modifiers, _ann) = self.parse_modifiers(n);
        let name = self.text(self.field(n, "name").unwrap());
        let mut implements = Vec::new();
        for c in self.all_children(n) {
            if c.kind() == "super_interfaces" {
                for tl in self.named_children(c) {
                    for t in self.named_children(tl) {
                        implements.push(self.typ(t));
                    }
                }
            }
        }
        let body = self.field(n, "body").unwrap();
        let mut entries = Vec::new();
        let mut members = Vec::new();
        for c in self.named_children(body) {
            match c.kind() {
                "enum_constant" => entries.push(self.enum_constant(c)),
                "enum_body_declarations" => {
                    // The leading `;` separates constants from body declarations
                    // and is consumed by the grammar (not an empty member).
                    let mut skipped_separator = false;
                    for m in self.all_children(c) {
                        match m.kind() {
                            "{" | "}" | "line_comment" | "block_comment" => {}
                            ";" if !skipped_separator => skipped_separator = true,
                            ";" => members.push(self.alloc(Node::EmptyMemberDeclaration, m)),
                            _ => members.push(self.member(m)),
                        }
                    }
                }
                _ => {}
            }
        }
        self.alloc(
            Node::EnumDeclaration {
                modifiers,
                name,
                implements,
                entries,
                members,
            },
            n,
        )
    }

    fn enum_constant(&mut self, n: TsNode<'a>) -> NodeId {
        let name = self.text(self.field(n, "name").unwrap());
        let args = match self.field(n, "arguments") {
            Some(a) => self.argument_list(a),
            None => Vec::new(),
        };
        let class_body = match self.field(n, "body") {
            Some(b) => self.members_of(b),
            None => Vec::new(),
        };
        self.alloc(
            Node::EnumConstantDeclaration {
                name,
                args,
                class_body,
            },
            n,
        )
    }

    fn members_of(&mut self, body: TsNode<'a>) -> Vec<NodeId> {
        let mut out = Vec::new();
        for c in self.all_children(body) {
            match c.kind() {
                "{" | "}" => {}
                ";" => out.push(self.alloc(Node::EmptyMemberDeclaration, c)),
                "line_comment" | "block_comment" => {}
                _ => out.push(self.member(c)),
            }
        }
        out
    }

    fn member(&mut self, n: TsNode<'a>) -> NodeId {
        match n.kind() {
            "field_declaration" => self.field_declaration(n),
            "method_declaration" => self.method_declaration(n),
            "constructor_declaration" => self.constructor_declaration(n),
            "class_declaration" => self.class_declaration(n, false),
            "interface_declaration" => self.class_declaration(n, true),
            "enum_declaration" => self.enum_declaration(n),
            "static_initializer" => {
                let blk = self
                    .named_children(n)
                    .into_iter()
                    .find(|c| c.kind() == "block")
                    .unwrap();
                let block = self.block(blk);
                self.alloc(Node::InitializerDeclaration { is_static: true, block }, n)
            }
            "block" => {
                let block = self.block(n);
                self.alloc(Node::InitializerDeclaration { is_static: false, block }, n)
            }
            _ => self.unsupported("member", n),
        }
    }

    fn field_declaration(&mut self, n: TsNode<'a>) -> NodeId {
        let (modifiers, _ann) = self.parse_modifiers(n);
        let typ = self.typ(self.field(n, "type").unwrap());
        let variables = self
            .fields(n, "declarator")
            .into_iter()
            .map(|d| self.variable_declarator(d))
            .collect();
        self.alloc(
            Node::FieldDeclaration {
                modifiers,
                typ,
                variables,
            },
            n,
        )
    }

    fn variable_declarator(&mut self, n: TsNode<'a>) -> NodeId {
        let name_node = self.field(n, "name").unwrap();
        let id = self.alloc(
            Node::VariableDeclaratorId {
                name: self.text(name_node),
            },
            name_node,
        );
        let init = self.field(n, "value").map(|v| self.expr(v));
        self.alloc(Node::VariableDeclarator { id, init }, n)
    }

    fn method_declaration(&mut self, n: TsNode<'a>) -> NodeId {
        let (modifiers, annotations) = self.parse_modifiers(n);
        let typ = self.typ(self.field(n, "type").unwrap());
        let name = self.text(self.field(n, "name").unwrap());
        let type_parameters = match self.field(n, "type_parameters") {
            Some(tp) => self.type_parameters(tp),
            None => Vec::new(),
        };
        let parameters = self.formal_parameters(self.field(n, "parameters").unwrap());
        let throws = self.throws_of(n);
        let body = self.field(n, "body").map(|b| self.block(b));
        let array_count = self
            .field(n, "dimensions")
            .map(|d| self.count_dims(d))
            .unwrap_or(0);
        let is_default = self.all_children(n).iter().any(|c| c.kind() == "default");
        self.alloc(
            Node::MethodDeclaration {
                modifiers,
                typ,
                name,
                type_parameters,
                parameters,
                throws,
                body,
                array_count,
                is_default,
                annotations,
            },
            n,
        )
    }

    fn constructor_declaration(&mut self, n: TsNode<'a>) -> NodeId {
        let (modifiers, _ann) = self.parse_modifiers(n);
        let name = self.text(self.field(n, "name").unwrap());
        let type_parameters = match self.field(n, "type_parameters") {
            Some(tp) => self.type_parameters(tp),
            None => Vec::new(),
        };
        let parameters = self.formal_parameters(self.field(n, "parameters").unwrap());
        let throws = self.throws_of(n);
        let block = self.block(self.field(n, "body").unwrap());
        self.alloc(
            Node::ConstructorDeclaration {
                modifiers,
                name,
                type_parameters,
                parameters,
                throws,
                block,
            },
            n,
        )
    }

    fn throws_of(&mut self, n: TsNode<'a>) -> Vec<NodeId> {
        let mut out = Vec::new();
        for c in self.all_children(n) {
            if c.kind() == "throws" {
                for t in self.named_children(c) {
                    out.push(self.reference_type(t));
                }
            }
        }
        out
    }

    fn formal_parameters(&mut self, n: TsNode<'a>) -> Vec<NodeId> {
        let mut out = Vec::new();
        for c in self.named_children(n) {
            match c.kind() {
                "formal_parameter" => out.push(self.formal_parameter(c, false)),
                "spread_parameter" => out.push(self.formal_parameter(c, true)),
                _ => {}
            }
        }
        out
    }

    fn formal_parameter(&mut self, n: TsNode<'a>, is_var_args: bool) -> NodeId {
        let (modifiers, _ann) = self.parse_modifiers(n);
        let typ = self.field(n, "type").map(|t| self.typ(t));
        // spread_parameter holds a variable_declarator; formal_parameter a `name` field.
        let id = if let Some(name_node) = self.field(n, "name") {
            self.alloc(
                Node::VariableDeclaratorId {
                    name: self.text(name_node),
                },
                name_node,
            )
        } else {
            // spread_parameter: find variable_declarator
            let vd = self
                .named_children(n)
                .into_iter()
                .find(|c| c.kind() == "variable_declarator")
                .unwrap();
            let nm = self.field(vd, "name").unwrap();
            self.alloc(
                Node::VariableDeclaratorId {
                    name: self.text(nm),
                },
                nm,
            )
        };
        let typ = if typ.is_none() {
            // spread_parameter type lives as a field on the node too
            self.field(n, "type").map(|t| self.typ(t))
        } else {
            typ
        };
        self.alloc(
            Node::Parameter {
                modifiers,
                typ,
                id,
                is_var_args,
            },
            n,
        )
    }

    fn count_dims(&self, dims: TsNode) -> i32 {
        // `dimensions` node contains one `[` `]` pair per dimension.
        let mut cur = dims.walk();
        dims.children(&mut cur).filter(|c| c.kind() == "[").count() as i32
    }

    // ---- type parameters ----

    fn type_parameters(&mut self, n: TsNode<'a>) -> Vec<NodeId> {
        let mut out = Vec::new();
        for c in self.named_children(n) {
            if c.kind() == "type_parameter" {
                let name = self.text(self.field(c, "name").unwrap_or_else(|| {
                    self.named_children(c).into_iter().next().unwrap()
                }));
                let mut type_bound = Vec::new();
                for tb in self.all_children(c) {
                    if tb.kind() == "type_bound" {
                        for t in self.named_children(tb) {
                            type_bound.push(self.typ(t));
                        }
                    }
                }
                out.push(self.alloc(Node::TypeParameter { name, type_bound }, c));
            }
        }
        out
    }

    // ---- types ----

    fn typ(&mut self, n: TsNode<'a>) -> NodeId {
        match n.kind() {
            "integral_type" | "floating_point_type" | "boolean_type" => {
                let kind = primitive_kind(&self.text(n));
                self.alloc(Node::PrimitiveType { kind }, n)
            }
            "void_type" => self.alloc(Node::VoidType, n),
            "array_type" => self.reference_type(n),
            "type_identifier" | "scoped_type_identifier" | "generic_type" => {
                let inner = self.class_or_interface_type(n);
                inner
            }
            _ => self.unsupported("type", n),
        }
    }

    /// JavaParser wraps non-primitive types (and arrays) in a ReferenceType.
    fn reference_type(&mut self, n: TsNode<'a>) -> NodeId {
        if n.kind() == "array_type" {
            let element = self.field(n, "element").unwrap();
            let dims = self.field(n, "dimensions");
            let array_count = dims.map(|d| self.count_dims(d)).unwrap_or(1);
            let typ = self.bare_type(element);
            self.alloc(Node::ReferenceType { typ, array_count }, n)
        } else {
            let typ = self.bare_type(n);
            self.alloc(Node::ReferenceType { typ, array_count: 0 }, n)
        }
    }

    /// The underlying type without ReferenceType wrapping (used inside arrays).
    fn bare_type(&mut self, n: TsNode<'a>) -> NodeId {
        match n.kind() {
            "integral_type" | "floating_point_type" | "boolean_type" => {
                let kind = primitive_kind(&self.text(n));
                self.alloc(Node::PrimitiveType { kind }, n)
            }
            "void_type" => self.alloc(Node::VoidType, n),
            _ => self.class_or_interface_type(n),
        }
    }

    fn class_or_interface_type(&mut self, n: TsNode<'a>) -> NodeId {
        match n.kind() {
            "scoped_type_identifier" => {
                // scope is the leading type, name is last identifier
                let children = self.named_children(n);
                let scope_node = children.first().copied();
                let scope = scope_node.map(|s| self.class_or_interface_type(s));
                let name = self.text(
                    self.all_children(n)
                        .into_iter()
                        .rev()
                        .find(|c| c.kind() == "type_identifier")
                        .unwrap(),
                );
                self.alloc(
                    Node::ClassOrInterfaceType {
                        scope,
                        name,
                        type_args: Vec::new(),
                        using_diamond: false,
                    },
                    n,
                )
            }
            "generic_type" => {
                let base = self.named_children(n)[0];
                let name = self.text(base);
                let mut type_args = Vec::new();
                let mut using_diamond = false;
                for c in self.named_children(n) {
                    if c.kind() == "type_arguments" {
                        let args = self.named_children(c);
                        if args.is_empty() {
                            using_diamond = true;
                        }
                        for a in args {
                            type_args.push(self.typ(a));
                        }
                    }
                }
                self.alloc(
                    Node::ClassOrInterfaceType {
                        scope: None,
                        name,
                        type_args,
                        using_diamond,
                    },
                    n,
                )
            }
            _ => {
                let name = self.text(n);
                self.alloc(
                    Node::ClassOrInterfaceType {
                        scope: None,
                        name,
                        type_args: Vec::new(),
                        using_diamond: false,
                    },
                    n,
                )
            }
        }
    }

    // ---- statements ----

    fn block(&mut self, n: TsNode<'a>) -> NodeId {
        let mut stmts = Vec::new();
        for c in self.all_children(n) {
            match c.kind() {
                "{" | "}" | "line_comment" | "block_comment" => {}
                ";" => stmts.push(self.alloc(Node::EmptyStmt, c)),
                _ => stmts.push(self.stmt(c)),
            }
        }
        self.alloc(Node::BlockStmt { stmts }, n)
    }

    fn stmt(&mut self, n: TsNode<'a>) -> NodeId {
        match n.kind() {
            "block" => self.block(n),
            "expression_statement" => {
                let inner = self.named_children(n)[0];
                let expression = self.expr(inner);
                self.alloc(Node::ExpressionStmt { expression }, n)
            }
            "local_variable_declaration" => {
                let vde = self.variable_declaration_expr(n);
                self.alloc(Node::ExpressionStmt { expression: vde }, n)
            }
            "return_statement" => {
                let expr = self.named_children(n).into_iter().next().map(|e| self.expr(e));
                self.alloc(Node::ReturnStmt { expr }, n)
            }
            "if_statement" => self.if_statement(n),
            "while_statement" => {
                let condition = self.expr(self.unwrap_paren(self.field(n, "condition").unwrap()));
                let body = self.stmt(self.field(n, "body").unwrap());
                self.alloc(Node::WhileStmt { condition, body }, n)
            }
            "do_statement" => {
                let body = self.stmt(self.field(n, "body").unwrap());
                let condition = self.expr(self.unwrap_paren(self.field(n, "condition").unwrap()));
                self.alloc(Node::DoStmt { body, condition }, n)
            }
            "for_statement" => self.for_statement(n),
            "enhanced_for_statement" => self.enhanced_for(n),
            "break_statement" => {
                let id = self
                    .named_children(n)
                    .into_iter()
                    .find(|c| c.kind() == "identifier")
                    .map(|c| self.text(c));
                self.alloc(Node::BreakStmt { id }, n)
            }
            "continue_statement" => {
                let id = self
                    .named_children(n)
                    .into_iter()
                    .find(|c| c.kind() == "identifier")
                    .map(|c| self.text(c));
                self.alloc(Node::ContinueStmt { id }, n)
            }
            "throw_statement" => {
                let expr = self.expr(self.named_children(n)[0]);
                self.alloc(Node::ThrowStmt { expr }, n)
            }
            "switch_expression" => self.switch_statement(n),
            "labeled_statement" => {
                let label = self.text(self.named_children(n)[0]);
                let stmt = self.stmt(self.named_children(n)[1]);
                self.alloc(Node::LabeledStmt { label, stmt }, n)
            }
            "synchronized_statement" => {
                let expr = self.expr(self.unwrap_paren(self.named_children(n)[0]));
                let block = self.block(self.field(n, "body").unwrap());
                self.alloc(Node::SynchronizedStmt { expr, block }, n)
            }
            "assert_statement" => {
                let kids = self.named_children(n);
                let check = self.expr(kids[0]);
                let message = kids.get(1).map(|&m| self.expr(m));
                self.alloc(Node::AssertStmt { check, message }, n)
            }
            ";" => self.alloc(Node::EmptyStmt, n),
            _ => self.unsupported("statement", n),
        }
    }

    fn switch_statement(&mut self, n: TsNode<'a>) -> NodeId {
        let selector = self.expr(self.unwrap_paren(self.field(n, "condition").unwrap()));
        let body = self.field(n, "body").unwrap();
        let mut entries = Vec::new();
        for grp in self.named_children(body) {
            if grp.kind() != "switch_block_statement_group" {
                self.unsupported("switch entry", grp);
            }
            // Collect labels (case/default) and the statements that follow them.
            let mut labels = Vec::new();
            let mut stmt_nodes = Vec::new();
            for c in self.all_children(grp) {
                match c.kind() {
                    "switch_label" => labels.push(c),
                    ":" | "{" | "}" => {}
                    "line_comment" | "block_comment" => {}
                    ";" => stmt_nodes.push(None),
                    _ => stmt_nodes.push(Some(c)),
                }
            }
            // JavaParser emits one SwitchEntryStmt per label; the statements go
            // with the last label of the group, the rest get empty bodies.
            let last = labels.len().saturating_sub(1);
            for (i, &lbl) in labels.iter().enumerate() {
                let label = self.named_children(lbl).into_iter().next().map(|e| self.expr(e));
                let stmts = if i == last {
                    stmt_nodes
                        .iter()
                        .map(|o| match o {
                            Some(s) => self.stmt(*s),
                            None => self.alloc(Node::EmptyStmt, grp),
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                entries.push(self.alloc(Node::SwitchEntryStmt { label, stmts }, lbl));
            }
        }
        self.alloc(Node::SwitchStmt { selector, entries }, n)
    }

    fn if_statement(&mut self, n: TsNode<'a>) -> NodeId {
        let condition = self.expr(self.unwrap_paren(self.field(n, "condition").unwrap()));
        let then_stmt = self.stmt(self.field(n, "consequence").unwrap());
        let else_stmt = self.field(n, "alternative").map(|a| self.stmt(a));
        self.alloc(
            Node::IfStmt {
                condition,
                then_stmt,
                else_stmt,
            },
            n,
        )
    }

    fn for_statement(&mut self, n: TsNode<'a>) -> NodeId {
        // tree-sitter for_statement: optional init, condition, update, body
        let mut init = Vec::new();
        if let Some(i) = self.field(n, "init") {
            if i.kind() == "local_variable_declaration" {
                init.push(self.variable_declaration_expr(i));
            } else {
                init.push(self.expr(i));
            }
        } else {
            // multiple init expressions appear as `init` fields
            for i in self.fields(n, "init") {
                init.push(self.expr(i));
            }
        }
        let compare = self.field(n, "condition").map(|c| self.expr(c));
        let update: Vec<NodeId> = self.fields(n, "update").into_iter().map(|u| self.expr(u)).collect();
        let body = self.stmt(self.field(n, "body").unwrap());
        self.alloc(
            Node::ForStmt {
                init,
                compare,
                update,
                body,
            },
            n,
        )
    }

    fn enhanced_for(&mut self, n: TsNode<'a>) -> NodeId {
        // for (Type x : iterable) body  -> ForeachStmt { variable: VariableDeclarationExpr }
        let typ = self.typ(self.field(n, "type").unwrap());
        let name_node = self.field(n, "name").unwrap();
        let vid = self.alloc(
            Node::VariableDeclaratorId {
                name: self.text(name_node),
            },
            name_node,
        );
        let vd = self.alloc(Node::VariableDeclarator { id: vid, init: None }, name_node);
        let variable = self.alloc(
            Node::VariableDeclarationExpr {
                modifiers: 0,
                typ,
                vars: vec![vd],
            },
            n,
        );
        let iterable = self.expr(self.field(n, "value").unwrap());
        let body = self.stmt(self.field(n, "body").unwrap());
        self.alloc(
            Node::ForeachStmt {
                variable,
                iterable,
                body,
            },
            n,
        )
    }

    fn variable_declaration_expr(&mut self, n: TsNode<'a>) -> NodeId {
        let (modifiers, _ann) = self.parse_modifiers(n);
        let typ = self.typ(self.field(n, "type").unwrap());
        let vars = self
            .fields(n, "declarator")
            .into_iter()
            .map(|d| self.variable_declarator(d))
            .collect();
        self.alloc(
            Node::VariableDeclarationExpr {
                modifiers,
                typ,
                vars,
            },
            n,
        )
    }

    /// `if`/`while`/`do` conditions consume the surrounding parentheses in the
    /// grammar; JavaParser stores the inner expression (one level unwrapped).
    fn unwrap_paren(&self, n: TsNode<'a>) -> TsNode<'a> {
        if n.kind() == "parenthesized_expression" {
            self.named_children(n).into_iter().next().unwrap_or(n)
        } else {
            n
        }
    }

    // ---- expressions ----

    fn argument_list(&mut self, n: TsNode<'a>) -> Vec<NodeId> {
        self.named_children(n).into_iter().map(|c| self.expr(c)).collect()
    }

    fn expr(&mut self, n: TsNode<'a>) -> NodeId {
        match n.kind() {
            "identifier" => {
                let name = self.text(n);
                self.alloc(Node::NameExpr { name }, n)
            }
            "decimal_integer_literal" | "hex_integer_literal" | "octal_integer_literal"
            | "binary_integer_literal" => {
                let value = self.text(n);
                if value.ends_with('l') || value.ends_with('L') {
                    self.alloc(Node::LongLiteralExpr { value }, n)
                } else {
                    self.alloc(Node::IntegerLiteralExpr { value }, n)
                }
            }
            "decimal_floating_point_literal" | "hex_floating_point_literal" => {
                let value = self.text(n);
                self.alloc(Node::DoubleLiteralExpr { value }, n)
            }
            "string_literal" => {
                let raw = self.text(n);
                let value = strip_quotes(&raw);
                self.alloc(Node::StringLiteralExpr { value }, n)
            }
            "character_literal" => {
                let raw = self.text(n);
                let value = raw.trim_matches('\'').to_string();
                self.alloc(Node::CharLiteralExpr { value }, n)
            }
            "true" => self.alloc(Node::BooleanLiteralExpr { value: true }, n),
            "false" => self.alloc(Node::BooleanLiteralExpr { value: false }, n),
            "null_literal" => self.alloc(Node::NullLiteralExpr, n),
            "this" => self.alloc(Node::ThisExpr { class_expr: None }, n),
            "super" => self.alloc(Node::SuperExpr { class_expr: None }, n),
            "parenthesized_expression" => {
                let inner = self.named_children(n).into_iter().next().map(|c| self.expr(c));
                self.alloc(Node::EnclosedExpr { inner }, n)
            }
            "binary_expression" => self.binary_expression(n),
            "unary_expression" => self.unary_expression(n),
            "update_expression" => self.update_expression(n),
            "assignment_expression" => self.assignment_expression(n),
            "method_invocation" => self.method_invocation(n),
            "object_creation_expression" => self.object_creation(n),
            "array_access" => {
                let name = self.expr(self.field(n, "array").unwrap());
                let index = self.expr(self.field(n, "index").unwrap());
                self.alloc(Node::ArrayAccessExpr { name, index }, n)
            }
            "field_access" => {
                let scope = self.expr(self.field(n, "object").unwrap());
                let field = self.text(self.field(n, "field").unwrap());
                self.alloc(
                    Node::FieldAccessExpr {
                        scope,
                        type_args: Vec::new(),
                        field,
                    },
                    n,
                )
            }
            "cast_expression" => {
                let typ = self.typ(self.field(n, "type").unwrap());
                let expr = self.expr(self.field(n, "value").unwrap());
                self.alloc(Node::CastExpr { typ, expr }, n)
            }
            "ternary_expression" => {
                let condition = self.expr(self.field(n, "condition").unwrap());
                let then_expr = self.expr(self.field(n, "consequence").unwrap());
                let else_expr = self.expr(self.field(n, "alternative").unwrap());
                self.alloc(
                    Node::ConditionalExpr {
                        condition,
                        then_expr,
                        else_expr,
                    },
                    n,
                )
            }
            "array_initializer" => {
                let values = self.named_children(n).into_iter().map(|c| self.expr(c)).collect();
                self.alloc(Node::ArrayInitializerExpr { values }, n)
            }
            "array_creation_expression" => self.array_creation(n),
            "instanceof_expression" => {
                let expr = self.expr(self.field(n, "left").unwrap());
                let typ = self.typ(self.field(n, "right").unwrap());
                self.alloc(Node::InstanceOfExpr { expr, typ }, n)
            }
            "scoped_identifier" => self.name_expr_of(n),
            _ => self.unsupported("expression", n),
        }
    }

    fn binary_expression(&mut self, n: TsNode<'a>) -> NodeId {
        let left = self.expr(self.field(n, "left").unwrap());
        let right = self.expr(self.field(n, "right").unwrap());
        let op = binary_op(&self.text(self.field(n, "operator").unwrap()));
        self.alloc(Node::BinaryExpr { left, op, right }, n)
    }

    fn unary_expression(&mut self, n: TsNode<'a>) -> NodeId {
        let operand = self.expr(self.field(n, "operand").unwrap());
        let op = match self.text(self.field(n, "operator").unwrap()).as_str() {
            "+" => UnaryOp::Positive,
            "-" => UnaryOp::Negative,
            "~" => UnaryOp::Inverse,
            "!" => UnaryOp::Not,
            o => panic!("adapter: unary op {o}"),
        };
        self.alloc(Node::UnaryExpr { expr: operand, op }, n)
    }

    fn update_expression(&mut self, n: TsNode<'a>) -> NodeId {
        // ++x / x++ / --x / x--
        let children = self.all_children(n);
        let operand_node = self.field(n, "operand").unwrap_or(children[0]);
        let operand = self.expr(operand_node);
        let op_text = children
            .iter()
            .find(|c| matches!(self.text(**c).as_str(), "++" | "--"))
            .map(|c| self.text(*c))
            .unwrap();
        let prefix = children
            .first()
            .map(|c| matches!(self.text(*c).as_str(), "++" | "--"))
            .unwrap_or(false);
        let op = match (op_text.as_str(), prefix) {
            ("++", true) => UnaryOp::PreIncrement,
            ("++", false) => UnaryOp::PosIncrement,
            ("--", true) => UnaryOp::PreDecrement,
            ("--", false) => UnaryOp::PosDecrement,
            _ => unreachable!(),
        };
        self.alloc(Node::UnaryExpr { expr: operand, op }, n)
    }

    fn assignment_expression(&mut self, n: TsNode<'a>) -> NodeId {
        let target = self.expr(self.field(n, "left").unwrap());
        let value = self.expr(self.field(n, "right").unwrap());
        let op = match self.text(self.field(n, "operator").unwrap()).as_str() {
            "=" => crate::ast::AssignOp::Assign,
            "&=" => crate::ast::AssignOp::And,
            "|=" => crate::ast::AssignOp::Or,
            "^=" => crate::ast::AssignOp::Xor,
            "+=" => crate::ast::AssignOp::Plus,
            "-=" => crate::ast::AssignOp::Minus,
            "%=" => crate::ast::AssignOp::Rem,
            "/=" => crate::ast::AssignOp::Slash,
            "*=" => crate::ast::AssignOp::Star,
            "<<=" => crate::ast::AssignOp::LShift,
            ">>=" => crate::ast::AssignOp::RSignedShift,
            ">>>=" => crate::ast::AssignOp::RUnsignedShift,
            o => panic!("adapter: assign op {o}"),
        };
        self.alloc(Node::AssignExpr { target, op, value }, n)
    }

    fn method_invocation(&mut self, n: TsNode<'a>) -> NodeId {
        let scope = self.field(n, "object").map(|o| self.expr(o));
        let name = self.text(self.field(n, "name").unwrap());
        let args = self.argument_list(self.field(n, "arguments").unwrap());
        self.alloc(
            Node::MethodCallExpr {
                scope,
                type_args: Vec::new(),
                name,
                args,
            },
            n,
        )
    }

    fn object_creation(&mut self, n: TsNode<'a>) -> NodeId {
        let typ = self.typ(self.field(n, "type").unwrap());
        let args = match self.field(n, "arguments") {
            Some(a) => self.argument_list(a),
            None => Vec::new(),
        };
        let anonymous_body = self
            .named_children(n)
            .into_iter()
            .find(|c| c.kind() == "class_body")
            .map(|b| self.members_of(b));
        self.alloc(
            Node::ObjectCreationExpr {
                scope: None,
                typ,
                type_args: Vec::new(),
                args,
                anonymous_body,
            },
            n,
        )
    }

    fn array_creation(&mut self, n: TsNode<'a>) -> NodeId {
        let typ = self.typ(self.field(n, "type").unwrap());
        let mut dimensions = Vec::new();
        let mut array_count = 0;
        let mut initializer = None;
        for c in self.all_children(n) {
            match c.kind() {
                "dimensions_expr" => {
                    array_count += 1;
                    if let Some(e) = self.named_children(c).into_iter().next() {
                        dimensions.push(self.expr(e));
                    }
                }
                "dimensions" => array_count += self.count_dims(c),
                "array_initializer" => initializer = Some(self.expr(c)),
                _ => {}
            }
        }
        self.alloc(
            Node::ArrayCreationExpr {
                typ,
                type_args: Vec::new(),
                array_count,
                dimensions,
                initializer,
            },
            n,
        )
    }

    // ---- modifiers / annotations ----

    fn parse_modifiers(&mut self, n: TsNode<'a>) -> (i32, Vec<NodeId>) {
        let mut bits = 0;
        let mut annotations = Vec::new();
        if let Some(m) = self.named_children(n).into_iter().find(|c| c.kind() == "modifiers") {
            for c in self.all_children(m) {
                match c.kind() {
                    "marker_annotation" | "annotation" => {
                        annotations.push(self.annotation(c));
                    }
                    other => bits |= modifiers::keyword_bit(other),
                }
            }
        }
        (bits, annotations)
    }

    fn annotation(&mut self, n: TsNode<'a>) -> NodeId {
        let name_node = self.field(n, "name").unwrap();
        let name = self.name_expr_of(name_node);
        self.alloc(Node::AnnotationExpr { name }, n)
    }
}

fn primitive_kind(text: &str) -> PrimitiveKind {
    match text {
        "boolean" => PrimitiveKind::Boolean,
        "char" => PrimitiveKind::Char,
        "byte" => PrimitiveKind::Byte,
        "short" => PrimitiveKind::Short,
        "int" => PrimitiveKind::Int,
        "long" => PrimitiveKind::Long,
        "float" => PrimitiveKind::Float,
        "double" => PrimitiveKind::Double,
        _ => PrimitiveKind::Int,
    }
}

fn binary_op(op: &str) -> BinaryOp {
    match op {
        "||" => BinaryOp::Or,
        "&&" => BinaryOp::And,
        "|" => BinaryOp::BinOr,
        "&" => BinaryOp::BinAnd,
        "^" => BinaryOp::Xor,
        "==" => BinaryOp::Equals,
        "!=" => BinaryOp::NotEquals,
        "<" => BinaryOp::Less,
        ">" => BinaryOp::Greater,
        "<=" => BinaryOp::LessEquals,
        ">=" => BinaryOp::GreaterEquals,
        "<<" => BinaryOp::LShift,
        ">>" => BinaryOp::RSignedShift,
        ">>>" => BinaryOp::RUnsignedShift,
        "+" => BinaryOp::Plus,
        "-" => BinaryOp::Minus,
        "*" => BinaryOp::Times,
        "/" => BinaryOp::Divide,
        "%" => BinaryOp::Remainder,
        o => panic!("adapter: binary op {o}"),
    }
}

fn strip_quotes(raw: &str) -> String {
    // JavaParser StringLiteralExpr stores the content without surrounding quotes,
    // keeping escape sequences as written.
    let bytes = raw.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'"' {
        raw[1..raw.len() - 1].to_string()
    } else {
        raw.to_string()
    }
}
