//! Port of `RustDumpVisitor` and its `SourcePrinter`.

use crate::ast::{
    Arena, AssignOp, BinaryOp, JClass, Node, NodeId, PrimitiveKind, UnaryOp,
};
use crate::id_tracker::IdTracker;
use crate::modifiers;
use crate::naming::camel_to_snake_case;

/// The `arg: Object` threaded through JavaParser's visitor: absent, or a Type node.
pub type Arg = Option<NodeId>;

/// Port of `RustDumpVisitor.SourcePrinter`.
pub struct SourcePrinter {
    indentation: String,
    buf: String,
    level: usize,
    indented: bool,
    marks: Vec<usize>,
}

impl SourcePrinter {
    pub fn new(indentation: &str) -> Self {
        SourcePrinter {
            indentation: indentation.to_string(),
            buf: String::new(),
            level: 0,
            indented: false,
            marks: Vec::new(),
        }
    }
    pub fn indent(&mut self) {
        self.level += 1;
    }
    pub fn unindent(&mut self) {
        self.level -= 1;
    }
    fn make_indent(&mut self) {
        for _ in 0..self.level {
            self.buf.push_str(&self.indentation);
        }
    }
    pub fn print(&mut self, arg: &str) {
        if !self.indented {
            self.make_indent();
            self.indented = true;
        }
        self.buf.push_str(arg);
    }
    pub fn print_ln_s(&mut self, arg: &str) {
        self.print(arg);
        self.print_ln();
    }
    pub fn print_ln(&mut self) {
        self.buf.push('\n');
        self.indented = false;
    }
    pub fn get_source(&self) -> String {
        self.buf.clone()
    }
    pub fn push(&mut self) -> usize {
        self.marks.push(self.buf.len());
        self.marks.len()
    }
    pub fn get_mark(&self, mark: usize) -> String {
        self.buf[self.marks[mark - 1]..].to_string()
    }
    pub fn pop(&mut self) {
        let start = self.marks[self.marks.len() - 1];
        self.buf.truncate(start);
        self.marks.pop();
    }
    pub fn drop(&mut self) {
        self.marks.pop();
    }
}

/// Member filter used by `printMembers`.
#[derive(Clone, Copy)]
enum Filter {
    /// Methods/constructors/initializers — excludes fields and nested types.
    Method,
}

pub struct RustDumpVisitor<'a> {
    pub printer: SourcePrinter,
    arena: &'a Arena,
    id: &'a mut IdTracker,
    /// Mirrors the original's `commentOut` flag (always false in this build, so
    /// the `/* private */`-style emissions it guards never fire).
    #[allow(dead_code)]
    comment_out: bool,
    print_comments: bool,
    /// Instance field names (Java spelling) of the enclosing class, used to
    /// decide whether a method needs `&mut self`.
    class_field_names: std::collections::HashSet<String>,
    /// Declarations inferred nullable (emit `Option<T>`); see `nullability`.
    nullable: &'a std::collections::HashSet<NodeId>,
    /// When true, the value being emitted feeds an `Option<T>` slot, so a
    /// nullable read is kept as-is (no `.unwrap()`).
    expect_option: bool,
    /// When true, string literals are emitted raw (`"x"`, not `"x".to_string()`)
    /// — used for `match` patterns.
    raw_string: bool,
}

impl<'a> RustDumpVisitor<'a> {
    pub fn new(
        print_comments: bool,
        arena: &'a Arena,
        id: &'a mut IdTracker,
        nullable: &'a std::collections::HashSet<NodeId>,
    ) -> Self {
        RustDumpVisitor {
            printer: SourcePrinter::new("    "),
            arena,
            id,
            comment_out: false,
            print_comments,
            class_field_names: std::collections::HashSet::new(),
            nullable,
            expect_option: false,
            raw_string: false,
        }
    }

    // ---- nullability helpers ----

    fn decl_nullable(&self, decl: NodeId) -> bool {
        self.nullable.contains(&decl)
    }

    fn name_decl_nullable(&self, name: &str, at: NodeId) -> bool {
        self.id
            .find_declaration_node_for(self.arena, name, at)
            .map(|(_, d)| self.nullable.contains(&d))
            .unwrap_or(false)
    }

    /// Mirror of `nullability::expr_nullable` for the dumper.
    fn expr_nullable(&self, e: NodeId) -> bool {
        match self.arena.kind(e) {
            Node::NullLiteralExpr => true,
            Node::NameExpr { name } => self.name_decl_nullable(name, e),
            Node::MethodCallExpr { scope: None, name, .. } => self.name_decl_nullable(name, e),
            Node::EnclosedExpr { inner: Some(i) } => self.expr_nullable(*i),
            Node::CastExpr { expr, .. } => self.expr_nullable(*expr),
            Node::ConditionalExpr { then_expr, else_expr, .. } => {
                self.expr_nullable(*then_expr) || self.expr_nullable(*else_expr)
            }
            _ => false,
        }
    }

    /// A name referring to a non-primitive (non-Copy) declaration — reading it
    /// by value out of a borrow needs `.clone()`.
    fn is_non_copy_name(&self, e: NodeId) -> bool {
        if let Node::NameExpr { name } = self.arena.kind(e) {
            if let Some((Some(td), _)) = self.id.find_declaration_node_for(self.arena, name, e) {
                return !td.is_primitive;
            }
        }
        false
    }

    /// Emit a value in a move position, cloning if it is a non-Copy name read.
    fn emit_moved_value(&mut self, e: NodeId, arg: Arg) {
        self.visit(e, arg);
        if self.is_non_copy_name(e) {
            self.printer.print(".clone()");
        }
    }

    fn enclosing_method_nullable(&self, mut n: NodeId) -> bool {
        while let Some(p) = self.arena.parent(n) {
            if matches!(self.arena.kind(p), Node::MethodDeclaration { .. }) {
                return self.nullable.contains(&p);
            }
            n = p;
        }
        false
    }

    /// Emit `value` into an `Option<T>` slot: `None` / existing-Option as-is /
    /// `Some(value)` for a plain value.
    fn emit_into_option(&mut self, value: NodeId, arg: Arg) {
        if self.expr_nullable(value) {
            let saved = self.expect_option;
            self.expect_option = true;
            self.visit(value, arg);
            self.expect_option = saved;
        } else {
            self.printer.print("Some(");
            let saved = self.expect_option;
            self.expect_option = false;
            self.visit(value, arg);
            self.expect_option = saved;
            self.printer.print(")");
        }
    }

    pub fn get_source(&self) -> String {
        self.printer.get_source()
    }

    // ---- helpers ----

    fn kind(&self, id: NodeId) -> Node {
        self.arena.kind(id).clone()
    }

    fn is_type(&self, arg: Arg) -> bool {
        match arg {
            Some(a) => matches!(
                self.arena.kind(a),
                Node::PrimitiveType { .. }
                    | Node::ReferenceType { .. }
                    | Node::ClassOrInterfaceType { .. }
                    | Node::VoidType
                    | Node::WildcardType { .. }
                    | Node::UnknownType
                    | Node::IntersectionType { .. }
                    | Node::UnionType { .. }
            ),
            None => false,
        }
    }

    fn to_snake_if_necessary(&self, n: &str) -> String {
        let n = match n {
            "NaN" => "NAN",
            "NEGATIVE_INFINITY" => "NEG_INFINITY",
            "POSITIVE_INFINITY" => "INFINITY",
            "MIN_VALUE" => "MIN",
            "MAX_VALUE" => "MAX",
            other => other,
        };
        let first = n.chars().next().unwrap();
        let s = if first.is_lowercase() {
            camel_to_snake_case(n)
        } else {
            n.to_string()
        };
        escape_rust_keyword(s)
    }

    fn remove_plus_and_suffix(&self, mut value: String, suffixes: &[&str]) -> String {
        if value.starts_with('+') {
            value = value[1..].to_string();
        }
        if value.starts_with('.') {
            value = format!("0{value}");
        }
        if suffixes.iter().any(|s| ends_with_ignore_none(&value, s)) {
            value = value[..value.len() - 1].to_string();
        }
        if value.ends_with('.') {
            value = format!("{value}0");
        }
        value = value.replace("d.", ".");
        value
    }

    // ---- printModifiers ----

    fn print_modifiers(&mut self, m: i32) {
        // commentOut is always false in this configuration, so only the `pub`
        // emissions remain observable.
        if modifiers::is_protected(m) {
            self.printer.print("pub ");
        }
        if modifiers::is_public(m) {
            self.printer.print("pub ");
        }
    }

    fn print_members(&mut self, members: &[NodeId], arg: Arg, filter: Filter) {
        for &member in members {
            let keep = match filter {
                Filter::Method => !matches!(
                    self.arena.kind(member),
                    Node::FieldDeclaration { .. }
                        | Node::ClassOrInterfaceDeclaration { .. }
                        | Node::EnumDeclaration { .. }
                ),
            };
            if keep {
                self.printer.print_ln();
                self.visit(member, arg);
                self.printer.print_ln();
            }
        }
    }

    fn print_type_args(&mut self, args: &[NodeId], arg: Arg) {
        if !args.is_empty() {
            self.printer.print("<");
            for (i, &t) in args.iter().enumerate() {
                self.visit(t, arg);
                if i + 1 < args.len() {
                    self.printer.print(", ");
                }
            }
            self.printer.print(">");
        }
    }

    fn print_type_parameters(&mut self, args: &[NodeId], arg: Arg) {
        if !args.is_empty() {
            self.printer.print("<");
            for (i, &t) in args.iter().enumerate() {
                self.visit(t, arg);
                if i + 1 < args.len() {
                    self.printer.print(", ");
                }
            }
            self.printer.print(">");
        }
    }

    fn print_arguments(&mut self, args: &[NodeId], arg: Arg) {
        self.printer.print("(");
        for (i, &e) in args.iter().enumerate() {
            match self.arena.kind(e) {
                Node::NameExpr { name } => {
                    if let Some((Some(left), _)) =
                        self.id.find_declaration_node_for(self.arena, name, e)
                    {
                        if !left.is_primitive || left.array_count > 0 {
                            self.printer.print("&");
                        }
                    }
                }
                _ => {}
            }
            self.visit(e, arg);
            if i + 1 < args.len() {
                self.printer.print(", ");
            }
        }
        self.printer.print(")");
    }

    fn print_java_comment(&mut self, id: NodeId, arg: Arg) {
        if let Some(c) = self.arena.comment(id) {
            self.visit(c, arg);
        }
    }

    fn accept_and_cut(&mut self, n: NodeId, arg: Arg) -> String {
        let mark = self.printer.push();
        self.visit(n, arg);
        let result = self.printer.get_mark(mark);
        self.printer.pop();
        result
    }

    // ---- main dispatch ----

    pub fn visit(&mut self, id: NodeId, arg: Arg) {
        use Node::*;
        match self.kind(id) {
            CompilationUnit { .. } => self.visit_compilation_unit(id, arg),
            PackageDeclaration { name } => {
                self.print_java_comment(id, arg);
                self.printer.print("// package ");
                self.visit(name, arg);
                self.printer.print_ln_s(";");
                self.printer.print_ln();
                self.print_orphan_comments_ending(id);
            }
            NameExpr { name } => self.visit_name_expr(id, &name, arg),
            QualifiedNameExpr { qualifier, name } => {
                self.print_java_comment(id, arg);
                self.visit(qualifier, arg);
                self.printer.print("::");
                self.printer.print(&name);
                self.print_orphan_comments_ending(id);
            }
            ImportDeclaration {
                name,
                is_static,
                is_asterisk,
            } => {
                self.print_java_comment(id, arg);
                self.printer.print("use ");
                if is_static {
                    self.printer.print("/* static */");
                }
                self.visit(name, arg);
                if is_asterisk {
                    self.printer.print("::*");
                }
                self.printer.print_ln_s(";");
                self.print_orphan_comments_ending(id);
            }
            ClassOrInterfaceDeclaration { .. } => self.visit_class(id, arg),
            EmptyTypeDeclaration => {
                self.print_java_comment(id, arg);
                self.printer.print(";");
                self.print_orphan_comments_ending(id);
            }
            JavadocComment { content } => {
                // Emit as a normal block comment, not `/**` — a Rust doc comment
                // requires an item to follow, which isn't guaranteed here.
                self.printer.print("/*");
                self.printer.print(&content);
                self.printer.print_ln_s("*/");
            }
            ClassOrInterfaceType { .. } => self.visit_class_type(id, arg),
            TypeParameter { name, type_bound } => {
                self.print_java_comment(id, arg);
                self.printer.print(&name);
                if !type_bound.is_empty() {
                    self.printer.print(": ");
                    for (i, &c) in type_bound.iter().enumerate() {
                        self.visit(c, arg);
                        if i + 1 < type_bound.len() {
                            self.printer.print(" + ");
                        }
                    }
                }
            }
            PrimitiveType { kind } => {
                self.print_java_comment(id, arg);
                self.printer.print(match kind {
                    PrimitiveKind::Boolean => "bool",
                    PrimitiveKind::Byte => "i8",
                    PrimitiveKind::Char => "char",
                    PrimitiveKind::Double => "f64",
                    PrimitiveKind::Float => "f32",
                    PrimitiveKind::Int => "i32",
                    PrimitiveKind::Long => "i64",
                    PrimitiveKind::Short => "i16",
                });
            }
            ReferenceType { typ, array_count } => {
                self.print_java_comment(id, arg);
                for _ in 0..array_count {
                    self.printer.print("Vec<");
                }
                self.visit(typ, arg);
                for _ in 0..array_count {
                    self.printer.print(">");
                }
            }
            IntersectionType { elements } => {
                self.print_java_comment(id, arg);
                let mut first = true;
                for e in elements {
                    self.visit(e, arg);
                    if first {
                        first = false;
                    } else {
                        self.printer.print(" & ");
                    }
                }
            }
            UnionType { elements } => {
                self.print_java_comment(id, arg);
                let mut first = true;
                for e in elements {
                    self.visit(e, arg);
                    if first {
                        first = false;
                    } else {
                        self.printer.print(" | ");
                    }
                }
            }
            WildcardType { ext, sup } => {
                // `? extends T` / `? super T` -> the bound type; bare `?` -> `_`.
                self.print_java_comment(id, arg);
                match ext.or(sup) {
                    Some(b) => self.visit(b, arg),
                    None => self.printer.print("_"),
                }
            }
            UnknownType => {}
            VoidType => {
                self.print_java_comment(id, arg);
                self.printer.print("void");
            }
            FieldDeclaration { typ, variables, .. } => {
                self.print_orphan_comments_before_this_child_node(id);
                self.print_java_comment(id, arg);
                self.printer.print(" ");
                for (i, &var) in variables.iter().enumerate() {
                    self.visit(var, Some(typ));
                    if i + 1 < variables.len() {
                        self.printer.print(", ");
                    }
                }
                self.printer.print(";");
            }
            VariableDeclarator { .. } => self.visit_variable_declarator(id, arg),
            VariableDeclaratorId { name } => {
                self.print_java_comment(id, arg);
                let s = self.to_snake_if_necessary(&name);
                self.printer.print(&s);
            }
            ArrayInitializerExpr { .. } => self.visit_array_initializer(id, arg),
            ArrayCreationExpr { .. } => self.visit_array_creation(id, arg),
            ArrayAccessExpr { name, index } => {
                self.print_java_comment(id, arg);
                self.visit(name, arg);
                // Rust indices are usize; Java's are int.
                self.printer.print("[(");
                self.visit(index, arg);
                self.printer.print(") as usize]");
            }
            AssignExpr { .. } => self.visit_assign(id, arg),
            BinaryExpr { .. } => self.visit_binary(id, arg),
            CastExpr { typ, expr } => {
                self.print_java_comment(id, arg);
                self.visit(expr, arg);
                self.printer.print(" as ");
                self.visit(typ, arg);
            }
            ClassExpr { typ } => {
                self.print_java_comment(id, arg);
                self.printer.print("std::any::TypeId::of::<");
                self.visit(typ, arg);
                self.printer.print(">()");
            }
            ConditionalExpr {
                condition,
                then_expr,
                else_expr,
            } => {
                self.print_java_comment(id, arg);
                self.printer.print(" if ");
                self.visit(condition, arg);
                self.printer.print(" { ");
                self.visit(then_expr, arg);
                self.printer.print(" } else { ");
                self.visit(else_expr, arg);
                self.printer.print(" }");
            }
            EnclosedExpr { inner } => {
                self.print_java_comment(id, arg);
                self.printer.print("(");
                if let Some(i) = inner {
                    self.visit(i, arg);
                }
                self.printer.print(")");
            }
            FieldAccessExpr { .. } => self.visit_field_access(id, arg),
            InstanceOfExpr { typ, .. } => {
                // Rust has no general runtime type test; emit a compiling stub.
                self.print_java_comment(id, arg);
                self.printer.print("false /* instanceof ");
                let t = self.accept_and_cut(typ, None);
                self.printer.print(t.trim());
                self.printer.print(" */");
            }
            CharLiteralExpr { value } => {
                self.print_java_comment(id, arg);
                self.printer.print("'");
                self.printer.print(&value);
                self.printer.print("'");
            }
            DoubleLiteralExpr { value } => {
                self.print_java_comment(id, arg);
                // Rust has no hex float literals; compute the value as decimal.
                if let Some(dec) = hex_float_to_decimal(&value) {
                    self.printer.print(&dec);
                } else {
                    let mut value = value;
                    if !value.contains(['.', 'e', 'E', 'x', 'X']) {
                        value = format!("{value}.0");
                    }
                    let s = self.remove_plus_and_suffix(value, &["D", "d"]);
                    self.printer.print(&s);
                }
            }
            IntegerLiteralExpr { value } => {
                self.print_java_comment(id, arg);
                let output = self.remove_plus_and_suffix(value, &[]);
                if self.is_float_in_history(Some(id)) {
                    self.printer.print(&format!("{output}.0"));
                } else {
                    self.printer.print(&output);
                }
            }
            LongLiteralExpr { value } => {
                self.print_java_comment(id, arg);
                let s = self.remove_plus_and_suffix(value, &["l", "L"]);
                self.printer.print(&s);
            }
            IntegerLiteralMinValueExpr { value } | LongLiteralMinValueExpr { value } => {
                self.print_java_comment(id, arg);
                self.printer.print(&value);
            }
            StringLiteralExpr { value } => {
                self.print_java_comment(id, arg);
                // Java String is owned, so emit `"...".to_string()` so String-typed
                // bindings type-check — except in a match pattern, where a raw
                // `"..."` (str literal) is required. (Concatenation uses format!.)
                self.printer.print("\"");
                self.printer.print(&value);
                self.printer
                    .print(if self.raw_string { "\"" } else { "\".to_string()" });
            }
            BooleanLiteralExpr { value } => {
                self.print_java_comment(id, arg);
                self.printer.print(if value { "true" } else { "false" });
            }
            NullLiteralExpr => {
                self.print_java_comment(id, arg);
                self.printer.print("None");
            }
            ThisExpr { class_expr } => {
                self.print_java_comment(id, arg);
                if let Some(ce) = class_expr {
                    self.visit(ce, arg);
                } else if self.id.is_in_constructor() {
                    // The value being built (see visit_constructor).
                    self.printer.print("__self");
                } else {
                    self.printer.print("self");
                }
            }
            SuperExpr { class_expr } => {
                self.print_java_comment(id, arg);
                if let Some(ce) = class_expr {
                    self.visit(ce, arg);
                    self.printer.print(".");
                }
                self.printer.print("super");
            }
            MethodCallExpr { .. } => self.visit_method_call(id, arg),
            ObjectCreationExpr { .. } => self.visit_object_creation(id, arg),
            UnaryExpr { .. } => self.visit_unary(id, arg),
            ConstructorDeclaration { .. } => self.visit_constructor(id, arg),
            MethodDeclaration { .. } => self.visit_method(id, arg),
            Parameter { .. } => self.visit_parameter(id, arg),
            MultiTypeParameter { modifiers: m, typ, id: vid } => {
                self.print_modifiers(m);
                self.visit(typ, arg);
                self.printer.print(" ");
                self.visit(vid, arg);
            }
            ExplicitConstructorInvocationStmt { .. } => {
                // `this(...)` / `super(...)` — no Rust equivalent; drop it.
                self.print_java_comment(id, arg);
                self.printer.print("/* super/this constructor call omitted */");
            }
            VariableDeclarationExpr { modifiers: m, typ, vars } => {
                self.print_java_comment(id, arg);
                self.print_modifiers(m);
                self.printer.print(" ");
                // `int i = 0, j = 1;` -> separate `let` statements (`; `-joined).
                for (i, &v) in vars.iter().enumerate() {
                    self.visit(v, Some(typ));
                    if i + 1 < vars.len() {
                        self.printer.print("; ");
                    }
                }
            }
            TypeDeclarationStmt { type_declaration } => {
                self.print_java_comment(id, arg);
                self.visit(type_declaration, arg);
            }
            AssertStmt { check, message } => {
                self.print_java_comment(id, arg);
                self.printer.print("assert!(");
                self.visit(check, arg);
                if let Some(msg) = message {
                    self.printer.print(", \"{}\", ");
                    self.visit(msg, arg);
                }
                self.printer.print(");");
            }
            BlockStmt { .. } => self.visit_block(id, arg),
            LabeledStmt { label, stmt } => {
                self.print_java_comment(id, arg);
                self.printer.print("'");
                self.printer.print(&label);
                self.printer.print(": ");
                self.visit(stmt, arg);
            }
            EmptyStmt => {
                self.print_java_comment(id, arg);
                self.printer.print(";");
            }
            ExpressionStmt { expression } => {
                self.print_orphan_comments_before_this_child_node(id);
                self.print_java_comment(id, arg);
                self.visit(expression, arg);
                self.printer.print(";");
            }
            SwitchStmt { .. } => self.visit_switch(id, arg),
            SwitchEntryStmt { .. } => {} // handled inline by visit_switch
            BreakStmt { id: lbl } => {
                self.print_java_comment(id, arg);
                self.printer.print("break");
                if let Some(l) = lbl {
                    self.printer.print(" '");
                    self.printer.print(&l);
                }
                self.printer.print(";");
            }
            ReturnStmt { expr } => {
                self.print_java_comment(id, arg);
                self.printer.print("return");
                if let Some(e) = expr {
                    self.printer.print(" ");
                    let throws = self.id.has_throws();
                    if throws {
                        self.printer.print("Ok(");
                    }
                    if self.enclosing_method_nullable(id) {
                        self.emit_into_option(e, arg);
                    } else {
                        self.emit_moved_value(e, arg);
                    }
                    if throws {
                        self.printer.print(")");
                    }
                }
                self.printer.print(";");
            }
            EnumDeclaration { .. } => self.visit_enum(id, arg),
            EnumConstantDeclaration { .. } => self.visit_enum_constant(id, arg),
            EmptyMemberDeclaration => {
                // A stray `;` is not a valid Rust item; emit nothing.
                self.print_java_comment(id, arg);
            }
            InitializerDeclaration { .. } => {
                // Static/instance initializer blocks have no item-level Rust form.
                self.print_java_comment(id, arg);
                self.printer.print("// initializer block omitted");
            }
            IfStmt { .. } => self.visit_if(id, arg),
            WhileStmt { condition, body } => {
                self.print_java_comment(id, arg);
                self.printer.print("while ");
                self.visit(condition, arg);
                self.printer.print(" ");
                self.encapsulate_if_not_block(body, arg);
            }
            ContinueStmt { id: lbl } => {
                self.print_java_comment(id, arg);
                self.printer.print("continue");
                if let Some(l) = lbl {
                    self.printer.print(" '");
                    self.printer.print(&l);
                }
                self.printer.print(";");
            }
            DoStmt { body, condition } => {
                self.print_java_comment(id, arg);
                self.printer.print("loop { ");
                self.visit(body, arg);
                self.printer.print(" if !(");
                self.visit(condition, arg);
                self.printer.print(") { break; } }");
            }
            ForeachStmt { variable, iterable, body } => {
                self.print_java_comment(id, arg);
                self.printer.print("for ");
                // Bind the loop variable by value (clone the iterable) to match
                // Java's value semantics and keep the body's uses simple.
                let vname = self.foreach_var_name(variable);
                self.printer.print(&vname);
                self.printer.print(" in ");
                self.visit(iterable, arg);
                self.printer.print(".clone() ");
                self.encapsulate_if_not_block(body, arg);
            }
            ForStmt { .. } => self.visit_for(id, arg),
            ThrowStmt { expr } => {
                self.print_java_comment(id, arg);
                // `throw new SomeException(msg)` -> `panic!("{:?}", msg)`. Using the
                // constructor argument (not the exception type) keeps it compiling
                // even though the exception type isn't defined.
                self.printer.print("panic!(\"{:?}\", ");
                match self.arena.kind(expr) {
                    Node::ObjectCreationExpr { args, .. } if !args.is_empty() => {
                        let first = args[0];
                        self.visit(first, arg);
                    }
                    Node::ObjectCreationExpr { .. } => self.printer.print("\"exception\""),
                    _ => self.visit(expr, arg),
                }
                self.printer.print(");");
            }
            SynchronizedStmt { block, .. } => {
                // No direct Rust equivalent; run the body (lock dropped).
                self.print_java_comment(id, arg);
                self.printer.print("/* synchronized */ ");
                self.visit(block, arg);
            }
            TryStmt { .. } => self.visit_try(id, arg),
            CatchClause { param, catch_block } => {
                self.print_java_comment(id, arg);
                self.printer.print(" catch (");
                self.visit(param, arg);
                self.printer.print(") ");
                self.visit(catch_block, arg);
            }
            MemberValuePair { name, value } => {
                self.print_java_comment(id, arg);
                self.printer.print(&name);
                self.printer.print(" = ");
                self.visit(value, arg);
            }
            LineComment { content } => {
                if self.print_comments {
                    self.printer.print("//");
                    let tmp = content.replace('\r', " ").replace('\n', " ");
                    self.printer.print_ln_s(&tmp);
                }
            }
            BlockComment { content } => {
                if self.print_comments {
                    self.printer.print("/*");
                    self.printer.print(&content);
                    self.printer.print_ln_s("*/");
                }
            }
            LambdaExpr { .. } => self.visit_lambda(id, arg),
            MethodReferenceExpr { .. } => self.visit_method_ref(id, arg),
            TypeExpr { typ } => {
                self.print_java_comment(id, arg);
                if let Some(t) = typ {
                    self.visit(t, arg);
                }
            }
            AnnotationExpr { .. } => {}
        }
    }

    // ---- per-node helpers (the heavier methods) ----

    fn visit_compilation_unit(&mut self, id: NodeId, arg: Arg) {
        let (package, imports, types) = match self.kind(id) {
            Node::CompilationUnit { package, imports, types } => (package, imports, types),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        if let Some(p) = package {
            self.visit(p, arg);
        }
        let _ = imports; // imports are intentionally not emitted (commented out upstream)
        if self.id.has_throws() {
            self.printer.print_ln_s("use std::rc::*;");
            self.printer.print_ln_s("use java::exc::*;");
        }
        if !types.is_empty() {
            let n = types.len();
            for (i, &t) in types.iter().enumerate() {
                self.visit(t, arg);
                self.printer.print_ln();
                if i + 1 < n {
                    self.printer.print_ln();
                }
            }
        }
        self.print_orphan_comments_ending(id);
    }

    fn visit_name_expr(&mut self, id: NodeId, name: &str, arg: Arg) {
        self.print_java_comment(id, arg);
        let decl = self.id.find_declaration_node_for(self.arena, name, id);
        let nullable = decl.map(|(_, d)| self.nullable.contains(&d)).unwrap_or(false);
        if let Some((_, right)) = decl {
            if (self.is_non_static_field_declaration(right) && !self.id.is_in_constructor())
                || self.is_non_static_method_declaration(right)
            {
                self.printer.print("self.");
            }
        }
        let s = self.to_snake_if_necessary(name);
        self.printer.print(&s);
        // A nullable value used where the plain value is expected gets unwrapped.
        if nullable && !self.expect_option {
            self.printer.print(".unwrap()");
        }
        self.print_orphan_comments_ending(id);
    }

    fn visit_class(&mut self, id: NodeId, arg: Arg) {
        let (modifiers_v, is_interface, name, type_parameters, extends, implements, members) =
            match self.kind(id) {
                Node::ClassOrInterfaceDeclaration {
                    modifiers,
                    is_interface,
                    name,
                    type_parameters,
                    extends,
                    implements,
                    members,
                } => (modifiers, is_interface, name, type_parameters, extends, implements, members),
                _ => unreachable!(),
            };
        self.print_java_comment(id, arg);

        if is_interface {
            self.visit_trait(modifiers_v, &name, &type_parameters, &extends, &members, arg);
            return;
        }

        // Track this class's instance field names for `&mut self` decisions.
        let saved_fields = std::mem::take(&mut self.class_field_names);
        for &m in &members {
            if let Node::FieldDeclaration { modifiers, variables, .. } = self.arena.kind(m) {
                if !modifiers::is_static(*modifiers) {
                    for &var in variables {
                        if let Node::VariableDeclarator { id: vid, .. } = self.arena.kind(var) {
                            if let Node::VariableDeclaratorId { name } = self.arena.kind(*vid) {
                                self.class_field_names.insert(name.clone());
                            }
                        }
                    }
                }
            }
        }

        // ---- struct ----
        // Clone: so field values can be cloned out from behind `&self`.
        // Default: so generated `new(...) -> Self` can start from a default value.
        self.printer.print_ln_s("#[derive(Clone, Default)]");
        self.print_modifiers(modifiers_v);
        self.printer.print("struct ");
        self.printer.print(&name);
        self.print_type_parameters(&type_parameters, arg);
        // (Java `extends`/`implements` have no direct struct equivalent and are
        // dropped — inheritance is not modelled.)
        let _ = (&extends, &implements);
        self.printer.print_ln_s(" {");
        self.printer.indent();
        for &m in &members {
            if let Node::FieldDeclaration { modifiers, .. } = self.arena.kind(m) {
                if !modifiers::is_static(*modifiers) {
                    self.emit_struct_field(m, arg);
                }
            }
        }
        self.printer.unindent();
        self.printer.print_ln_s("}");
        self.printer.print_ln();

        // ---- impl ----
        self.printer.print("impl ");
        self.printer.print(&name);
        self.print_type_parameters(&type_parameters, arg);
        self.printer.print_ln_s(" {");
        self.printer.indent();
        // static fields as associated constants
        for &m in &members {
            if let Node::FieldDeclaration { modifiers, .. } = self.arena.kind(m) {
                if modifiers::is_static(*modifiers) {
                    self.emit_const_field(m, arg);
                }
            }
        }
        // methods / constructors / initializers (NOT nested types — Rust forbids
        // `struct`/`enum`/`trait` items inside an `impl`).
        self.print_members(&members, arg, Filter::Method);
        self.print_orphan_comments_ending(id);
        self.printer.unindent();
        self.printer.print_ln_s("}");

        // Nested type declarations are hoisted to module level.
        for &m in &members {
            if matches!(
                self.arena.kind(m),
                Node::ClassOrInterfaceDeclaration { .. } | Node::EnumDeclaration { .. }
            ) {
                self.printer.print_ln();
                self.visit(m, arg);
                self.printer.print_ln();
            }
        }
        self.class_field_names = saved_fields;
    }

    /// Does this subtree assign to an instance field (→ method needs `&mut self`)?
    fn mutates_self(&self, node: NodeId) -> bool {
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            match self.arena.kind(n) {
                Node::AssignExpr { target, .. } => {
                    if self.is_self_target(*target) {
                        return true;
                    }
                }
                Node::UnaryExpr { expr, op } => {
                    use crate::ast::UnaryOp::*;
                    if matches!(op, PreIncrement | PreDecrement | PosIncrement | PosDecrement)
                        && self.is_self_target(*expr)
                    {
                        return true;
                    }
                }
                _ => {}
            }
            for c in self.arena.children(n) {
                stack.push(c);
            }
        }
        false
    }

    fn is_self_target(&self, t: NodeId) -> bool {
        match self.arena.kind(t) {
            Node::NameExpr { name } => self.class_field_names.contains(name),
            Node::FieldAccessExpr { scope, field, .. } => {
                matches!(self.arena.kind(*scope), Node::ThisExpr { .. })
                    || self.class_field_names.contains(field)
            }
            Node::ArrayAccessExpr { name, .. } => self.is_self_target(*name),
            _ => false,
        }
    }

    fn visit_trait(
        &mut self,
        modifiers_v: i32,
        name: &str,
        type_parameters: &[NodeId],
        extends: &[NodeId],
        members: &[NodeId],
        arg: Arg,
    ) {
        self.print_modifiers(modifiers_v);
        self.printer.print("trait ");
        self.printer.print(name);
        self.print_type_parameters(type_parameters, arg);
        if !extends.is_empty() {
            self.printer.print(" : ");
            for (i, &e) in extends.iter().enumerate() {
                if i > 0 {
                    self.printer.print(" + ");
                }
                self.visit(e, arg);
            }
        }
        self.printer.print_ln_s(" {");
        self.printer.indent();
        // Methods only; nested types (and fields) can't live in a trait body.
        self.print_members(members, arg, Filter::Method);
        self.printer.unindent();
        self.printer.print_ln_s("}");
        // Hoist nested type declarations to module level.
        for &m in members {
            if matches!(
                self.arena.kind(m),
                Node::ClassOrInterfaceDeclaration { .. } | Node::EnumDeclaration { .. }
            ) {
                self.printer.print_ln();
                self.visit(m, arg);
                self.printer.print_ln();
            }
        }
    }

    /// Emit a non-static field as a Rust struct field: `name: Type,` (one per
    /// declared variable; Java field initializers are dropped here).
    fn emit_struct_field(&mut self, field_id: NodeId, _arg: Arg) {
        let (typ, variables) = match self.kind(field_id) {
            Node::FieldDeclaration { typ, variables, .. } => (typ, variables),
            _ => return,
        };
        for var in variables {
            let name = self.field_var_name(var);
            let nullable = self.var_decl_id(var).map(|d| self.decl_nullable(d)).unwrap_or(false);
            self.printer.print(&format!("{name}: "));
            if nullable {
                self.printer.print("Option<");
            }
            self.visit(typ, None);
            if nullable {
                self.printer.print(">");
            }
            self.printer.print_ln_s(",");
        }
    }

    /// The (snake-cased) loop variable name of a foreach's VariableDeclarationExpr.
    fn foreach_var_name(&self, variable: NodeId) -> String {
        if let Node::VariableDeclarationExpr { vars, .. } = self.arena.kind(variable) {
            if let Some(&v) = vars.first() {
                return self.field_var_name(v);
            }
        }
        String::new()
    }

    fn var_decl_id(&self, var: NodeId) -> Option<NodeId> {
        if let Node::VariableDeclarator { id, .. } = self.arena.kind(var) {
            Some(*id)
        } else {
            None
        }
    }

    /// Emit a static field as an associated `const`.
    fn emit_const_field(&mut self, field_id: NodeId, _arg: Arg) {
        let (typ, variables) = match self.kind(field_id) {
            Node::FieldDeclaration { typ, variables, .. } => (typ, variables),
            _ => return,
        };
        let type_str = self.accept_and_cut(typ, None);
        let type_str = type_str.trim().to_string();
        for var in variables {
            let name = self.field_var_name(var);
            self.printer.print(&format!("const {name}: "));
            self.visit(typ, None);
            self.printer.print(" = ");
            match self.arena.kind(var) {
                Node::VariableDeclarator { init: Some(i), .. } => {
                    let i = *i;
                    self.visit(i, None);
                }
                _ => {
                    let d = self.default_value(&type_str);
                    self.printer.print(&d);
                }
            }
            self.printer.print_ln_s(";");
        }
    }

    fn field_var_name(&self, var: NodeId) -> String {
        if let Node::VariableDeclarator { id, .. } = self.arena.kind(var) {
            if let Node::VariableDeclaratorId { name } = self.arena.kind(*id) {
                return self.to_snake_if_necessary(name);
            }
        }
        String::new()
    }

    fn visit_class_type(&mut self, id: NodeId, arg: Arg) {
        let (scope, name, type_args, using_diamond) = match self.kind(id) {
            Node::ClassOrInterfaceType { scope, name, type_args, using_diamond } => {
                (scope, name, type_args, using_diamond)
            }
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        if let Some(s) = scope {
            self.visit(s, arg);
            self.printer.print("::");
        }
        self.printer.print(map_type_name(&name));
        if using_diamond {
            // No empty turbofish in Rust; let the args be inferred.
        } else {
            self.print_type_args(&type_args, arg);
        }
    }

    fn visit_variable_declarator(&mut self, id: NodeId, arg: Arg) {
        let (vid, init) = match self.kind(id) {
            Node::VariableDeclarator { id: vid, init } => (vid, init),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        let name = self.accept_and_cut(vid, arg);
        let mut is_constant = false;
        if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
            self.printer.print("const ");
            is_constant = true;
        } else {
            self.printer.print("let ");
            if self.id.is_changed(self.arena, &name, id) {
                self.printer.print("mut ");
            }
        }
        self.printer.print(&name);
        let nullable = self.decl_nullable(vid);
        if self.is_type(arg) {
            self.printer.print(": ");
            let tmp = self.accept_and_cut(arg.unwrap(), None);
            let tmp = tmp.trim().to_string();
            if nullable {
                self.printer.print(&format!("Option<{tmp}>"));
            } else if is_constant && tmp == "String" {
                self.printer.print("&'static str");
            } else {
                self.printer.print(&tmp);
            }
        }
        if let Some(i) = init {
            self.printer.print(" = ");
            // Java allows `char c = 65;` (int->char); Rust needs an explicit cast.
            let char_from_int = self.is_char_type(arg)
                && matches!(self.arena.kind(i), Node::IntegerLiteralExpr { .. });
            if nullable {
                self.emit_into_option(i, arg);
            } else if char_from_int {
                self.visit(i, arg);
                self.printer.print(" as u8 as char");
            } else {
                self.emit_moved_value(i, arg);
            }
        }
    }

    fn is_char_type(&self, arg: Arg) -> bool {
        matches!(
            arg.map(|a| self.arena.kind(a)),
            Some(Node::PrimitiveType { kind: PrimitiveKind::Char })
        )
    }

    fn visit_array_initializer(&mut self, id: NodeId, arg: Arg) {
        let values = match self.kind(id) {
            Node::ArrayInitializerExpr { values } => values,
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        // A plain Vec literal; the binding's type is emitted by the declarator.
        self.printer.print("vec![");
        for &val in &values {
            self.visit(val, None);
            self.printer.print(", ");
        }
        self.printer.print("]");
    }

    fn default_value(&self, ty: &str) -> String {
        match ty {
            "f64" | "f32" => "0.0",
            "u64" | "u32" | "u16" | "u8" | "usize" | "i64" | "i32" | "i16" | "i8" => "0",
            "bool" => "false",
            _ => "None",
        }
        .to_string()
    }

    fn visit_array_creation(&mut self, id: NodeId, arg: Arg) {
        let (typ, dimensions, initializer) = match self.kind(id) {
            Node::ArrayCreationExpr {
                typ,
                dimensions,
                initializer,
                ..
            } => (typ, dimensions, initializer),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        if let Some(init) = initializer {
            // `new T[]{a, b}` -> `vec![a, b]`.
            self.visit(init, None);
        } else if !dimensions.is_empty() {
            // `new T[n]` -> `vec![<default>; (n) as usize]`, nested for `[a][b]`.
            let ty = self.accept_and_cut(typ, arg);
            let ty = ty.trim().to_string();
            let default = self.default_value(&ty);
            let mut s = default;
            for &d in dimensions.iter().rev() {
                let dim = self.accept_and_cut(d, arg);
                s = format!("vec![{s}; ({}) as usize]", dim.trim());
            }
            self.printer.print(&s);
        }
    }

    fn visit_assign(&mut self, id: NodeId, arg: Arg) {
        let (target, op, value) = match self.kind(id) {
            Node::AssignExpr { target, op, value } => (target, op, value),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        // Assigning to a nullable slot: keep the target as the bare Option (no
        // unwrap) and wrap the value with Some/None.
        let target_nullable = matches!(op, AssignOp::Assign) && self.expr_nullable(target);
        let saved = self.expect_option;
        self.expect_option = target_nullable;
        self.visit(target, arg);
        self.expect_option = saved;
        self.printer.print(" ");
        self.printer.print(match op {
            AssignOp::Assign => "=",
            AssignOp::And => "&=",
            AssignOp::Or => "|=",
            AssignOp::Xor => "^=",
            AssignOp::Plus => "+=",
            AssignOp::Minus => "-=",
            AssignOp::Rem => "%=",
            AssignOp::Slash => "/=",
            AssignOp::Star => "*=",
            AssignOp::LShift => "<<=",
            AssignOp::RSignedShift => ">>=",
            AssignOp::RUnsignedShift => ">>= /* >>>= */",
        });
        self.printer.print(" ");
        if target_nullable {
            self.emit_into_option(value, arg);
        } else if matches!(op, AssignOp::Assign) {
            self.emit_moved_value(value, arg);
        } else {
            self.visit(value, arg);
        }
    }

    fn visit_binary(&mut self, id: NodeId, arg: Arg) {
        if self.id.get_type(id) == Some(JClass::StringClass) {
            self.print_string_expression(id, arg);
            return;
        }
        let (left, op, right) = match self.kind(id) {
            Node::BinaryExpr { left, op, right } => (left, op, right),
            _ => unreachable!(),
        };
        // Null comparison -> Option::is_none()/is_some().
        if matches!(op, BinaryOp::Equals | BinaryOp::NotEquals) {
            let l_null = matches!(self.arena.kind(left), Node::NullLiteralExpr);
            let r_null = matches!(self.arena.kind(right), Node::NullLiteralExpr);
            if l_null ^ r_null {
                let other = if l_null { right } else { left };
                self.print_java_comment(id, arg);
                let saved = self.expect_option;
                self.expect_option = true;
                self.visit(other, arg);
                self.expect_option = saved;
                self.printer.print(if matches!(op, BinaryOp::Equals) {
                    ".is_none()"
                } else {
                    ".is_some()"
                });
                return;
            }
        }
        self.print_java_comment(id, arg);
        self.visit(left, arg);
        self.printer.print(" ");
        self.printer.print(match op {
            BinaryOp::Or => "||",
            BinaryOp::And => "&&",
            BinaryOp::BinOr => "|",
            BinaryOp::BinAnd => "&",
            BinaryOp::Xor => "^",
            BinaryOp::Equals => "==",
            BinaryOp::NotEquals => "!=",
            BinaryOp::Less => "<",
            BinaryOp::Greater => ">",
            BinaryOp::LessEquals => "<=",
            BinaryOp::GreaterEquals => ">=",
            BinaryOp::LShift => "<<",
            BinaryOp::RSignedShift => ">>",
            BinaryOp::RUnsignedShift => ">> /* >>> */",
            BinaryOp::Plus => "+",
            BinaryOp::Minus => "-",
            BinaryOp::Times => "*",
            BinaryOp::Divide => "/",
            BinaryOp::Remainder => "%",
        });
        self.printer.print(" ");
        self.visit(right, arg);
    }

    fn gen_string_expr_sequence(&self, id: NodeId, result: &mut Vec<NodeId>) {
        if let Node::BinaryExpr { left, op, right } = self.kind(id) {
            if op == BinaryOp::Plus {
                self.gen_string_part(left, result);
                self.gen_string_part(right, result);
                return;
            }
        }
        result.push(id);
    }

    fn gen_string_part(&self, n: NodeId, result: &mut Vec<NodeId>) {
        if matches!(self.arena.kind(n), Node::BinaryExpr { .. }) {
            self.gen_string_expr_sequence(n, result);
        } else {
            result.push(n);
        }
    }

    fn print_string_expression(&mut self, id: NodeId, arg: Arg) {
        let mut chain = Vec::new();
        self.gen_string_expr_sequence(id, &mut chain);
        self.printer.print("format!(\"");
        for &node in &chain {
            if let Node::StringLiteralExpr { value } = self.arena.kind(node) {
                let v = value.clone();
                self.printer.print(&v);
            } else {
                self.printer.print("{}");
            }
        }
        self.printer.print("\"");
        for &node in &chain {
            if !matches!(self.arena.kind(node), Node::StringLiteralExpr { .. }) && node != id {
                self.printer.print(", ");
                self.visit(node, arg);
            }
        }
        self.printer.print(")");
    }

    /// Is `s` a reference to a class/type (→ `::`), as opposed to a value
    /// (variable/field, → `.`)? A class name is an uppercase `NameExpr` that does
    /// not resolve to any declaration in scope.
    fn is_static_class_ref(&self, s: NodeId) -> bool {
        match self.arena.kind(s) {
            Node::NameExpr { name } => {
                name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                    && self.id.find_declaration_node_for(self.arena, name, s).is_none()
            }
            _ => false,
        }
    }

    fn visit_field_access(&mut self, id: NodeId, arg: Arg) {
        let (scope, field) = match self.kind(id) {
            Node::FieldAccessExpr { scope, field, .. } => (scope, field),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        self.visit(scope, arg);
        self.printer.print(if self.is_static_class_ref(scope) { "::" } else { "." });
        // `.length` -> `.len()`; otherwise snake-case + keyword-escape the field
        // to match how the field declaration is emitted.
        if field == "length" {
            self.printer.print("len()");
        } else {
            let f = self.to_snake_if_necessary(&field);
            self.printer.print(&f);
        }
    }

    fn visit_method_call(&mut self, id: NodeId, arg: Arg) {
        let (scope, type_args, name, args) = match self.kind(id) {
            Node::MethodCallExpr { scope, type_args, name, args } => (scope, type_args, name, args),
            _ => unreachable!(),
        };
        if self.try_emit_print_macro(scope, &name, &args, arg) {
            return;
        }
        if self.try_emit_math(scope, &name, &args, arg) {
            return;
        }
        if self.try_emit_int_range(scope, &name, &args, arg) {
            return;
        }
        if self.try_emit_optional_static(scope, &name, &args, arg) {
            return;
        }
        if self.try_emit_string_format(scope, &name, &args, arg) {
            return;
        }
        if self.try_emit_known_method(scope, &name, &args, arg) {
            return;
        }
        self.print_java_comment(id, arg);
        if let Some(s) = scope {
            self.visit(s, arg);
            self.printer.print(if self.is_static_class_ref(s) { "::" } else { "." });
        }
        // Explicit method type arguments are dropped (Rust infers them; emitting
        // `::<T>name` here would be invalid).
        let _ = &type_args;
        if scope.is_none() {
            if let Some((_, right)) = self.id.find_declaration_node_for(self.arena, &name, id) {
                match self.arena.kind(right) {
                    Node::MethodDeclaration { modifiers: m, .. } => {
                        if !modifiers::is_static(*m) {
                            self.printer.print("self.");
                        } else {
                            self.printer.print("::");
                        }
                    }
                    _ => self.printer.print("self."),
                }
            }
        }
        let s = self.to_snake_if_necessary(&name);
        self.printer.print(&s);
        self.print_arguments(&args, arg);
        // A call to a nullable-returning method used as a plain value is unwrapped.
        if scope.is_none() && !self.expect_option && self.name_decl_nullable(&name, id) {
            self.printer.print(".unwrap()");
        }
    }

    /// Map `System.out.println(x)` / `System.err.print(x)` etc. to the Rust
    /// print macros. Returns true if it handled the call.
    fn try_emit_print_macro(&mut self, scope: Option<NodeId>, name: &str, args: &[NodeId], arg: Arg) -> bool {
        let Some(s) = scope else { return false };
        let Node::FieldAccessExpr { scope: inner, field, .. } = self.arena.kind(s) else {
            return false;
        };
        let (inner, field) = (*inner, field.clone());
        if !matches!(self.arena.kind(inner), Node::NameExpr { name } if name == "System") {
            return false;
        }
        let mac = match (field.as_str(), name) {
            ("out", "println") => "println",
            ("err", "println") => "eprintln",
            ("out", "print") => "print",
            ("err", "print") => "eprint",
            _ => return false,
        };
        self.printer.print(mac);
        self.printer.print("!(");
        if let Some(&first) = args.first() {
            self.printer.print("\"{}\", ");
            self.visit(first, arg);
        }
        self.printer.print(")");
        true
    }

    /// `Optional.of(x)` -> `Some(x)`, `Optional.empty()` -> `None`.
    fn try_emit_optional_static(&mut self, scope: Option<NodeId>, name: &str, args: &[NodeId], arg: Arg) -> bool {
        let Some(s) = scope else { return false };
        if !matches!(self.arena.kind(s), Node::NameExpr { name } if name == "Optional") {
            return false;
        }
        match (name, args.len()) {
            ("of", 1) | ("ofNullable", 1) => {
                self.printer.print("Some(");
                self.visit(args[0], arg);
                self.printer.print(")");
                true
            }
            ("empty", 0) => {
                self.printer.print("None");
                true
            }
            _ => false,
        }
    }

    /// `IntStream.range(a, b)` -> `((a)..(b))`, `rangeClosed` -> `..=`.
    fn try_emit_int_range(&mut self, scope: Option<NodeId>, name: &str, args: &[NodeId], arg: Arg) -> bool {
        let Some(s) = scope else { return false };
        let is_intstream = matches!(
            self.arena.kind(s),
            Node::NameExpr { name } if name == "IntStream" || name == "LongStream"
        );
        if !is_intstream || args.len() != 2 {
            return false;
        }
        let sep = match name {
            "range" => "..",
            "rangeClosed" => "..=",
            _ => return false,
        };
        self.printer.print("((");
        self.visit(args[0], arg);
        self.printer.print(")");
        self.printer.print(sep);
        self.printer.print("(");
        self.visit(args[1], arg);
        self.printer.print("))");
        true
    }

    /// `String.format("%d ...", a, ...)` -> `format!("{} ...", a, ...)`.
    fn try_emit_string_format(&mut self, scope: Option<NodeId>, name: &str, args: &[NodeId], arg: Arg) -> bool {
        let Some(s) = scope else { return false };
        if name != "format"
            || !matches!(self.arena.kind(s), Node::NameExpr { name } if name == "String")
        {
            return false;
        }
        let Some(&fmt_node) = args.first() else { return false };
        let Node::StringLiteralExpr { value } = self.arena.kind(fmt_node) else {
            return false;
        };
        let converted = java_format_to_rust(value);
        self.printer.print("format!(\"");
        self.printer.print(&converted);
        self.printer.print("\"");
        for &a in &args[1..] {
            self.printer.print(", ");
            self.visit(a, arg);
        }
        self.printer.print(")");
        true
    }

    /// The declared Java type's simple name for the variable `name` refers to.
    fn decl_java_type_name(&self, name: &str, at: NodeId) -> Option<String> {
        let (_, decl) = self.id.find_declaration_node_for(self.arena, name, at)?;
        let parent = self.arena.parent(decl)?;
        let grand = self.arena.parent(parent);
        let typ = match self.arena.kind(parent) {
            Node::Parameter { typ, .. } => *typ,
            _ => match grand.map(|g| self.arena.kind(g)) {
                Some(Node::FieldDeclaration { typ, .. })
                | Some(Node::VariableDeclarationExpr { typ, .. }) => Some(*typ),
                _ => None,
            },
        }?;
        self.type_simple_name(typ)
    }

    fn type_simple_name(&self, typ: NodeId) -> Option<String> {
        match self.arena.kind(typ) {
            Node::ClassOrInterfaceType { name, .. } => Some(name.clone()),
            Node::ReferenceType { typ, .. } => self.type_simple_name(*typ),
            _ => None,
        }
    }

    /// Map common collection / String methods to their Rust equivalents.
    fn try_emit_known_method(&mut self, scope: Option<NodeId>, name: &str, args: &[NodeId], arg: Arg) -> bool {
        let Some(recv) = scope else { return false };
        match (name, args.len()) {
            ("size", 0) | ("length", 0) => {
                self.printer.print("(");
                self.visit(recv, arg);
                self.printer.print(".len() as i32)");
                true
            }
            ("isEmpty", 0) => {
                // Optional.isEmpty -> is_none; collection isEmpty -> is_empty.
                let opt = matches!(self.recv_type_name(recv).as_deref(), Some("Optional"));
                self.visit(recv, arg);
                self.printer.print(if opt { ".is_none()" } else { ".is_empty()" });
                true
            }
            // ---- Optional ----
            ("isPresent", 0) => self.emit_recv_method(recv, "is_some", arg),
            ("orElse", 1) => {
                self.visit(recv, arg);
                self.printer.print(".unwrap_or(");
                self.visit(args[0], arg);
                self.printer.print(")");
                true
            }
            ("orElseGet", 1) => {
                self.visit(recv, arg);
                self.printer.print(".unwrap_or_else(");
                self.visit(args[0], arg);
                self.printer.print(")");
                true
            }
            ("get", 0) => self.emit_recv_method(recv, "unwrap", arg),
            // ---- reduce / sorted ----
            ("reduce", 2) => {
                self.visit(recv, arg);
                self.printer.print(".fold(");
                self.visit(args[0], arg);
                self.printer.print(", ");
                self.visit(args[1], arg);
                self.printer.print(")");
                true
            }
            ("sorted", 0) => {
                self.printer.print("{ let mut __s = ");
                self.visit(recv, arg);
                self.printer.print(".collect::<Vec<_>>(); __s.sort(); __s.into_iter() }");
                true
            }
            ("equals", 1) => {
                self.printer.print("(");
                self.visit(recv, arg);
                self.printer.print(" == ");
                self.visit(args[0], arg);
                self.printer.print(")");
                true
            }
            ("add", 1) => {
                // Set.add -> insert; List/Collection.add -> push.
                let is_set = matches!(
                    self.recv_type_name(recv).as_deref(),
                    Some("Set" | "HashSet" | "LinkedHashSet" | "TreeSet")
                );
                self.visit(recv, arg);
                self.printer.print(if is_set { ".insert(" } else { ".push(" });
                self.visit(args[0], arg);
                self.printer.print(")");
                true
            }
            ("get", 1) => {
                let is_map = matches!(
                    self.recv_type_name(recv).as_deref(),
                    Some("Map" | "HashMap" | "LinkedHashMap" | "TreeMap")
                );
                if is_map {
                    // Map.get(k) -> value by clone (panics if absent, ~ Java null deref).
                    self.visit(recv, arg);
                    self.printer.print(".get(&(");
                    self.visit(args[0], arg);
                    self.printer.print(")).cloned().unwrap()");
                } else {
                    // List.get(i) -> indexed element (cloned to own it).
                    self.visit(recv, arg);
                    self.printer.print("[(");
                    self.visit(args[0], arg);
                    self.printer.print(") as usize].clone()");
                }
                true
            }
            ("put", 2) => {
                self.visit(recv, arg);
                self.printer.print(".insert(");
                self.visit(args[0], arg);
                self.printer.print(", ");
                self.visit(args[1], arg);
                self.printer.print(")");
                true
            }
            ("contains", 1) => {
                let is_string = matches!(self.recv_type_name(recv).as_deref(), Some("String"));
                self.visit(recv, arg);
                if is_string {
                    self.printer.print(".contains((");
                    self.visit(args[0], arg);
                    self.printer.print(").as_str())");
                } else {
                    self.printer.print(".contains(&(");
                    self.visit(args[0], arg);
                    self.printer.print("))");
                }
                true
            }
            // ---- more streams ----
            ("findFirst", 0) | ("findAny", 0) => {
                self.visit(recv, arg);
                self.printer.print(".next()");
                true
            }
            ("limit", 1) => self.emit_iter_count(recv, "take", args[0], arg),
            ("skip", 1) => self.emit_iter_count(recv, "skip", args[0], arg),
            ("sum", 0) => {
                self.visit(recv, arg);
                self.printer.print(".sum::<i32>()");
                true
            }
            // ---- more String ops (arg is a String -> &str) ----
            ("startsWith", 1) => self.emit_str_arg(recv, "starts_with", args[0], arg),
            ("endsWith", 1) => self.emit_str_arg(recv, "ends_with", args[0], arg),
            ("replace", 2) => {
                self.visit(recv, arg);
                self.printer.print(".replace((");
                self.visit(args[0], arg);
                self.printer.print(").as_str(), (");
                self.visit(args[1], arg);
                self.printer.print(").as_str())");
                true
            }
            ("split", 1) => {
                self.visit(recv, arg);
                self.printer.print(".split((");
                self.visit(args[0], arg);
                self.printer.print(").as_str()).map(|x| x.to_string()).collect::<Vec<_>>()");
                true
            }
            ("indexOf", 1) => {
                self.visit(recv, arg);
                self.printer.print(".find((");
                self.visit(args[0], arg);
                self.printer.print(").as_str()).map(|i| i as i32).unwrap_or(-1)");
                true
            }
            ("containsKey", 1) => {
                self.visit(recv, arg);
                self.printer.print(".contains_key(&(");
                self.visit(args[0], arg);
                self.printer.print("))");
                true
            }
            // ---- streams ----
            ("stream", 0) => {
                // Owned-value iterator so map/forEach closures see `T`, not `&T`.
                self.visit(recv, arg);
                self.printer.print(".iter().cloned()");
                true
            }
            ("toArray", 0) => {
                self.visit(recv, arg);
                self.printer.print(".collect::<Vec<_>>()");
                true
            }
            ("collect", _) => {
                // Inspect the Collector: joining -> join, toSet -> HashSet.
                let collector = args.first().and_then(|&a| match self.arena.kind(a) {
                    Node::MethodCallExpr { scope: Some(s), name, args, .. }
                        if matches!(self.arena.kind(*s), Node::NameExpr { name } if name == "Collectors") =>
                    {
                        Some((name.clone(), args.clone()))
                    }
                    _ => None,
                });
                match collector {
                    Some((m, cargs)) if m == "joining" => {
                        self.visit(recv, arg);
                        self.printer
                            .print(".map(|x| x.to_string()).collect::<Vec<_>>().join(");
                        if let Some(&sep) = cargs.first() {
                            self.printer.print("(");
                            self.visit(sep, arg);
                            self.printer.print(").as_str()");
                        } else {
                            self.printer.print("\"\"");
                        }
                        self.printer.print(")");
                    }
                    Some((m, _)) if m == "toSet" => {
                        self.visit(recv, arg);
                        self.printer.print(".collect::<std::collections::HashSet<_>>()");
                    }
                    _ => {
                        self.visit(recv, arg);
                        self.printer.print(".collect::<Vec<_>>()");
                    }
                }
                true
            }
            ("mapToInt", 1) | ("mapToLong", 1) | ("mapToDouble", 1) | ("mapToObj", 1) => {
                self.visit(recv, arg);
                self.printer.print(".map(");
                self.visit(args[0], arg);
                self.printer.print(")");
                true
            }
            // Predicate combinators borrow each item (`&T`), but the Java lambda
            // treats it as `T` — clone-shadow inside the closure to bridge that.
            ("filter", 1) => self.emit_predicate(recv, "filter", args[0], arg),
            ("anyMatch", 1) => self.emit_predicate(recv, "any", args[0], arg),
            ("allMatch", 1) => self.emit_predicate(recv, "all", args[0], arg),
            ("count", 0) => {
                self.printer.print("(");
                self.visit(recv, arg);
                self.printer.print(".count() as i32)");
                true
            }
            ("toLowerCase", 0) => self.emit_recv_method(recv, "to_lowercase", arg),
            ("toUpperCase", 0) => self.emit_recv_method(recv, "to_uppercase", arg),
            ("trim", 0) => {
                self.visit(recv, arg);
                self.printer.print(".trim().to_string()");
                true
            }
            ("charAt", 1) => {
                self.visit(recv, arg);
                self.printer.print(".chars().nth((");
                self.visit(args[0], arg);
                self.printer.print(") as usize).unwrap()");
                true
            }
            ("substring", 1) => {
                self.visit(recv, arg);
                self.printer.print("[(");
                self.visit(args[0], arg);
                self.printer.print(") as usize..].to_string()");
                true
            }
            ("substring", 2) => {
                self.visit(recv, arg);
                self.printer.print("[(");
                self.visit(args[0], arg);
                self.printer.print(") as usize..(");
                self.visit(args[1], arg);
                self.printer.print(") as usize].to_string()");
                true
            }
            _ => false,
        }
    }

    /// Emit `recv.<method>(|p| { let p = p.clone(); <body> })` for a borrowing
    /// predicate combinator whose argument is a one-parameter lambda.
    fn emit_predicate(&mut self, recv: NodeId, method: &str, pred: NodeId, arg: Arg) -> bool {
        let (params, body) = match self.arena.kind(pred) {
            Node::LambdaExpr { parameters, body, .. } if parameters.len() == 1 => {
                (parameters.clone(), *body)
            }
            // Non-lambda predicate (e.g. method ref): fall back to a plain call.
            _ => {
                self.visit(recv, arg);
                self.printer.print(".");
                self.printer.print(method);
                self.printer.print("(");
                self.visit(pred, arg);
                self.printer.print(")");
                return true;
            }
        };
        let p = self.param_name(params[0]);
        self.visit(recv, arg);
        self.printer.print(".");
        self.printer.print(method);
        self.printer
            .print(&format!("(|{p}| {{ let {p} = {p}.clone(); "));
        if let Node::ExpressionStmt { expression } = self.arena.kind(body) {
            let e = *expression;
            self.visit(e, arg);
        } else {
            self.visit(body, arg);
        }
        self.printer.print(" })");
        true
    }

    /// `recv.<method>((arg) as usize)` — for iterator take/skip.
    fn emit_iter_count(&mut self, recv: NodeId, method: &str, n: NodeId, arg: Arg) -> bool {
        self.visit(recv, arg);
        self.printer.print(&format!(".{method}(("));
        self.visit(n, arg);
        self.printer.print(") as usize)");
        true
    }

    /// `recv.<method>((arg).as_str())` — for String methods taking a &str.
    fn emit_str_arg(&mut self, recv: NodeId, method: &str, s: NodeId, arg: Arg) -> bool {
        self.visit(recv, arg);
        self.printer.print(&format!(".{method}(("));
        self.visit(s, arg);
        self.printer.print(").as_str())");
        true
    }

    fn emit_recv_method(&mut self, recv: NodeId, method: &str, arg: Arg) -> bool {
        self.visit(recv, arg);
        self.printer.print(".");
        self.printer.print(method);
        self.printer.print("()");
        true
    }

    fn recv_type_name(&self, recv: NodeId) -> Option<String> {
        if let Node::NameExpr { name } = self.arena.kind(recv) {
            self.decl_java_type_name(name, recv)
        } else {
            None
        }
    }

    /// Map `Math.x(...)` to a Rust receiver method, e.g. `Math.max(a, b)` ->
    /// `(a).max(b)`, `Math.sqrt(x)` -> `(x).sqrt()`. Returns true if handled.
    fn try_emit_math(&mut self, scope: Option<NodeId>, name: &str, args: &[NodeId], arg: Arg) -> bool {
        let Some(s) = scope else { return false };
        if !matches!(self.arena.kind(s), Node::NameExpr { name } if name == "Math") {
            return false;
        }
        // (receiver-method, arity)
        let m = match name {
            "abs" | "sqrt" | "floor" | "ceil" | "round" | "signum" => (name, 1),
            "max" | "min" => (name, 2),
            "pow" => ("powf", 2),
            _ => return false,
        };
        if args.len() != m.1 {
            return false;
        }
        self.printer.print("(");
        self.visit(args[0], arg);
        self.printer.print(").");
        self.printer.print(m.0);
        self.printer.print("(");
        if m.1 == 2 {
            self.visit(args[1], arg);
        }
        self.printer.print(")");
        true
    }

    fn visit_object_creation(&mut self, id: NodeId, arg: Arg) {
        let (scope, typ, type_args, args, anonymous_body) = match self.kind(id) {
            Node::ObjectCreationExpr { scope, typ, type_args, args, anonymous_body } => {
                (scope, typ, type_args, args, anonymous_body)
            }
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        if let Some(s) = scope {
            self.visit(s, arg);
            self.printer.print(".");
        }
        let _ = type_args;
        // Emit `<MappedType>::new(...)`, dropping the diamond/type-args. Known
        // collections are constructed with no arguments.
        let base = match self.arena.kind(typ) {
            Node::ClassOrInterfaceType { name, .. } => map_type_name(name).to_string(),
            _ => self.accept_and_cut(typ, arg).trim().to_string(),
        };
        self.printer.print(&base);
        self.printer.print("::new");
        if is_rust_collection(&base) {
            self.printer.print("()");
        } else {
            self.print_arguments(&args, arg);
        }
        // Anonymous class bodies have no inline Rust equivalent; drop them.
        if anonymous_body.is_some() {
            self.printer.print(" /* anonymous class body omitted */");
        }
    }

    fn is_embedded_in_stmt(&self, id: NodeId) -> bool {
        match self.arena.parent(id).map(|p| self.arena.kind(p)) {
            Some(Node::ExpressionStmt { .. }) | Some(Node::ForStmt { .. }) => false,
            _ => true,
        }
    }

    fn org_visit_unary(&mut self, id: NodeId, arg: Arg) {
        let (expr, op) = match self.kind(id) {
            Node::UnaryExpr { expr, op } => (expr, op),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        match op {
            UnaryOp::Positive => self.printer.print("+"),
            UnaryOp::Negative => self.printer.print("-"),
            UnaryOp::Inverse => self.printer.print("!"), // Java `~` is Rust `!` on ints
            UnaryOp::Not => self.printer.print("!"),
            UnaryOp::PreIncrement => self.printer.print("++"),
            UnaryOp::PreDecrement => self.printer.print("--"),
            _ => {}
        }
        self.visit(expr, arg);
        match op {
            UnaryOp::PosIncrement => self.printer.print("++"),
            UnaryOp::PosDecrement => self.printer.print("--"),
            _ => {}
        }
    }

    fn visit_unary(&mut self, id: NodeId, arg: Arg) {
        let (expr, op) = match self.kind(id) {
            Node::UnaryExpr { expr, op } => (expr, op),
            _ => unreachable!(),
        };
        use UnaryOp::*;
        match op {
            PreIncrement | PreDecrement | PosIncrement | PosDecrement => {
                self.print_java_comment(id, arg);
                let opstr = if matches!(op, PreDecrement | PosDecrement) {
                    " -= 1"
                } else {
                    " += 1"
                };
                let post = matches!(op, PosIncrement | PosDecrement);
                if self.is_embedded_in_stmt(id) {
                    // Used as a value: lower to a block expression.
                    if post {
                        // x++ : yield old value
                        self.printer.print("{ let __v = ");
                        self.visit(expr, arg);
                        self.printer.print("; ");
                        self.visit(expr, arg);
                        self.printer.print(opstr);
                        self.printer.print("; __v }");
                    } else {
                        // ++x : increment then yield
                        self.printer.print("{ ");
                        self.visit(expr, arg);
                        self.printer.print(opstr);
                        self.printer.print("; ");
                        self.visit(expr, arg);
                        self.printer.print(" }");
                    }
                } else {
                    // Statement context: a plain compound assignment.
                    self.visit(expr, arg);
                    self.printer.print(opstr);
                }
            }
            Positive => {
                self.print_java_comment(id, arg);
                self.visit(expr, arg);
            }
            // Negative / Not / Inverse: prefix operator.
            _ => self.org_visit_unary(id, arg),
        }
    }

    fn visit_constructor(&mut self, id: NodeId, arg: Arg) {
        let (modifiers_v, name, type_parameters, parameters, throws, block) = match self.kind(id) {
            Node::ConstructorDeclaration {
                modifiers, name, type_parameters, parameters, throws, block,
            } => (modifiers, name, type_parameters, parameters, throws, block),
            _ => unreachable!(),
        };
        self.id.set_in_constructor(true);
        self.print_java_comment(id, arg);
        self.print_modifiers(modifiers_v);
        self.print_type_parameters(&type_parameters, arg);
        if !type_parameters.is_empty() {
            self.printer.print(" ");
        }
        let _ = throws; // Java `throws` has no Rust equivalent here.
        self.printer.print("fn new");
        self.printer.print("(");
        for (i, &p) in parameters.iter().enumerate() {
            self.visit(p, arg);
            if i + 1 < parameters.len() {
                self.printer.print(", ");
            }
        }
        self.printer.print(") -> ");
        self.printer.print(&name);
        // Build the value in `__self` (`this` maps to it), then return it.
        self.printer.print_ln_s(" {");
        self.printer.indent();
        self.printer
            .print_ln_s(&format!("let mut __self: {name} = Default::default();"));
        if let Node::BlockStmt { stmts } = self.kind(block) {
            for s in stmts {
                self.visit(s, arg);
                self.printer.print_ln();
            }
        }
        self.printer.print_ln_s("return __self;");
        self.printer.unindent();
        self.printer.print_ln_s("}");
        self.id.set_in_constructor(false);
    }

    fn visit_method(&mut self, id: NodeId, arg: Arg) {
        let (modifiers_v, typ, name, type_parameters, parameters, throws, body, array_count, is_default, annotations) =
            match self.kind(id) {
                Node::MethodDeclaration {
                    modifiers, typ, name, type_parameters, parameters, throws, body, array_count, is_default, annotations,
                } => (modifiers, typ, name, type_parameters, parameters, throws, body, array_count, is_default, annotations),
                _ => unreachable!(),
            };
        self.id.set_current_method(Some(name.clone()));
        self.print_orphan_comments_before_this_child_node(id);
        self.print_java_comment(id, arg);
        for a in &annotations {
            if let Node::AnnotationExpr { name: an } = self.arena.kind(*a) {
                if self.annotation_simple_name(*an) == "Test" {
                    self.printer.print_ln_s("#[test]");
                }
            }
        }
        let _ = is_default; // Rust default methods are just methods.
        self.print_modifiers(modifiers_v);
        self.printer.print("fn ");
        let raw_type = self.accept_and_cut(typ, arg);
        let ret_nullable = self.decl_nullable(id) && raw_type.trim() != "void";
        let type_string = if ret_nullable {
            format!("Option<{}>", raw_type.trim())
        } else {
            raw_type.clone()
        };
        let snake = self.to_snake_if_necessary(&name);
        self.printer.print(&snake);
        // Type parameters go after the name in Rust: `fn name<T>(...)`.
        self.print_type_parameters(&type_parameters, arg);
        self.printer.print("(");
        if !modifiers::is_static(modifiers_v) {
            let needs_mut = body.map(|b| self.mutates_self(b)).unwrap_or(false);
            self.printer.print(if needs_mut { "&mut self" } else { "&self" });
            if !parameters.is_empty() {
                self.printer.print(", ");
            }
        }
        for (i, &p) in parameters.iter().enumerate() {
            self.visit(p, arg);
            if i + 1 < parameters.len() {
                self.printer.print(", ");
            }
        }
        self.printer.print(") ");
        if type_string != "void" {
            self.printer.print("-> ");
            if array_count > 0 {
                self.printer.print("/* ");
                for _ in 0..array_count {
                    self.printer.print("[]");
                }
                self.printer.print(" */");
            }
            if !throws.is_empty() {
                self.replace_throws(&throws, arg, &type_string);
            } else {
                self.printer.print(&type_string);
            }
        } else if !throws.is_empty() {
            self.printer.print(" -> ");
            self.replace_throws(&throws, arg, "Void");
        }
        self.printer.print(" ");
        match body {
            None => self.printer.print(";"),
            Some(b) => {
                self.printer.print(" ");
                self.visit(b, arg);
            }
        }
        self.id.set_current_method(None);
    }

    fn replace_throws(&mut self, throws: &[NodeId], arg: Arg, type_string: &str) {
        self.printer.print("/* ");
        self.printer.print(" throws ");
        for (i, &r) in throws.iter().enumerate() {
            self.visit(r, arg);
            if i + 1 < throws.len() {
                self.printer.print(", ");
            }
        }
        self.printer.print(" */");
        self.printer.print("Result<");
        self.printer.print(type_string);
        self.printer.print(", Rc<Exception>> ");
    }

    fn visit_parameter(&mut self, id: NodeId, arg: Arg) {
        let (typ, vid, is_var_args) = match self.kind(id) {
            Node::Parameter { typ, id: vid, is_var_args, .. } => (typ, vid, is_var_args),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        self.printer.print(" ");
        self.visit(vid, arg);
        self.printer.print(": ");
        let nullable = self.decl_nullable(vid);
        if is_var_args {
            // Java varargs `T... xs` -> `xs: Vec<T>`.
            self.printer.print("Vec<");
            if let Some(t) = typ {
                self.visit(t, arg);
            }
            self.printer.print(">");
            return;
        }
        let is_primitive = typ
            .map(|t| matches!(self.arena.kind(t), Node::PrimitiveType { .. }))
            .unwrap_or(false);
        if nullable {
            // Option<T> owns its value; no borrow.
            self.printer.print("Option<");
            if let Some(t) = typ {
                self.visit(t, arg);
            }
            self.printer.print(">");
        } else {
            if !is_primitive {
                self.printer.print("&");
            }
            if let Some(t) = typ {
                self.visit(t, arg);
            }
        }
    }

    fn visit_block(&mut self, id: NodeId, arg: Arg) {
        let stmts = match self.kind(id) {
            Node::BlockStmt { stmts } => stmts,
            _ => unreachable!(),
        };
        self.print_orphan_comments_before_this_child_node(id);
        self.print_java_comment(id, arg);
        self.printer.print_ln_s("{");
        self.printer.indent();
        for &s in &stmts {
            self.visit(s, arg);
            self.printer.print_ln();
        }
        self.printer.unindent();
        self.print_orphan_comments_ending(id);
        self.printer.print("}");
    }

    fn visit_switch(&mut self, id: NodeId, arg: Arg) {
        let (selector, entries) = match self.kind(id) {
            Node::SwitchStmt { selector, entries } => (selector, entries),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        // Matching on Java String requires `match sel.as_str() { "a" => ... }`.
        let string_switch = entries.iter().any(|&e| {
            matches!(self.arena.kind(e),
                Node::SwitchEntryStmt { label: Some(l), .. }
                if matches!(self.arena.kind(*l), Node::StringLiteralExpr { .. }))
        });
        self.printer.print("match ");
        if string_switch {
            self.printer.print("(");
            self.visit(selector, arg);
            self.printer.print(").as_str()");
        } else {
            self.visit(selector, arg);
        }
        self.printer.print_ln_s(" {");
        self.printer.indent();
        // Java case fall-through: consecutive labels with no body share the body
        // of the next labelled case. In Rust that is an or-pattern
        // `a | b | c => { ... }`. A `default` label becomes `_`.
        let mut pending: Vec<NodeId> = Vec::new(); // accumulated case-label exprs
        let mut pending_default = false;
        for &e in &entries {
            let (label, stmts) = match self.arena.kind(e) {
                Node::SwitchEntryStmt { label, stmts } => (*label, stmts.clone()),
                _ => continue,
            };
            match label {
                Some(l) => pending.push(l),
                None => pending_default = true,
            }
            if stmts.is_empty() {
                continue; // fall through to the next case
            }
            // Emit the accumulated pattern.
            self.emit_switch_patterns(&pending, pending_default, arg);
            self.printer.print_ln_s(" => {");
            self.printer.indent();
            for &s in &stmts {
                self.visit(s, arg);
                self.printer.print_ln();
            }
            self.printer.unindent();
            self.printer.print_ln_s("}");
            pending.clear();
            pending_default = false;
        }
        // Trailing labels with no body (e.g. an empty final case).
        if pending_default || !pending.is_empty() {
            self.emit_switch_patterns(&pending, pending_default, arg);
            self.printer.print_ln_s(" => {}");
        }
        self.printer.unindent();
        self.printer.print("}");
    }

    /// Emit a match pattern: `_` for default, else `p1 | p2 | …`. String labels
    /// are emitted raw (no `.to_string()`).
    fn emit_switch_patterns(&mut self, pending: &[NodeId], default: bool, arg: Arg) {
        if default {
            self.printer.print("_");
            return;
        }
        let saved = self.raw_string;
        self.raw_string = true;
        for (i, &l) in pending.iter().enumerate() {
            if i > 0 {
                self.printer.print(" | ");
            }
            self.visit(l, arg);
        }
        self.raw_string = saved;
    }

    fn visit_enum(&mut self, id: NodeId, arg: Arg) {
        let (modifiers_v, name, implements, entries, members) = match self.kind(id) {
            Node::EnumDeclaration { modifiers, name, implements, entries, members } => {
                (modifiers, name, implements, entries, members)
            }
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        self.print_modifiers(modifiers_v);
        self.printer.print("enum ");
        self.printer.print(&name);
        let _ = &implements; // `implements` has no Rust enum equivalent; dropped.
        self.printer.print_ln_s(" {");
        self.printer.indent();
        // Variants only. Java enum fields/constructors/methods have no direct
        // Rust enum equivalent and are dropped.
        let _ = &members;
        for &e in &entries {
            self.visit(e, arg);
            self.printer.print_ln_s(",");
        }
        self.printer.unindent();
        self.printer.print_ln_s("}");
    }

    fn visit_enum_constant(&mut self, id: NodeId, arg: Arg) {
        let (name, args, class_body) = match self.kind(id) {
            Node::EnumConstantDeclaration { name, args, class_body } => (name, args, class_body),
            _ => unreachable!(),
        };
        // A Rust enum variant is just a name; Java enum constructor args and
        // per-constant class bodies have no equivalent and are dropped.
        let _ = (&args, &class_body);
        self.print_java_comment(id, arg);
        self.printer.print(&name);
    }

    fn visit_if(&mut self, id: NodeId, arg: Arg) {
        let (condition, then_stmt, else_stmt) = match self.kind(id) {
            Node::IfStmt { condition, then_stmt, else_stmt } => (condition, then_stmt, else_stmt),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        self.printer.print("if ");
        self.visit(condition, arg);
        let then_block = matches!(self.arena.kind(then_stmt), Node::BlockStmt { .. });
        if then_block {
            self.printer.print(" ");
        } else {
            self.printer.print_ln_s(" {");
            self.printer.indent();
        }
        self.visit(then_stmt, arg);
        if !then_block {
            self.printer.unindent();
            self.printer.print_ln();
            self.printer.print_ln_s("}");
        }
        if let Some(else_s) = else_stmt {
            if then_block {
                self.printer.print(" ");
            }
            let else_if = matches!(self.arena.kind(else_s), Node::IfStmt { .. });
            let else_block = matches!(self.arena.kind(else_s), Node::BlockStmt { .. });
            if else_if || else_block {
                self.printer.print("else ");
            } else {
                self.printer.print("else {");
                self.printer.indent();
            }
            self.visit(else_s, arg);
            if !(else_if || else_block) {
                self.printer.unindent();
                self.printer.print_ln();
                self.printer.print_ln_s("}");
            }
        }
    }

    fn encapsulate_if_not_block(&mut self, n: NodeId, arg: Arg) {
        if matches!(self.arena.kind(n), Node::BlockStmt { .. }) {
            self.visit(n, arg);
        } else {
            self.printer.print_ln_s(" {");
            self.printer.indent();
            self.visit(n, arg);
            self.printer.print_ln_s("}");
        }
    }

    fn visit_for(&mut self, id: NodeId, arg: Arg) {
        let (init, compare, update, body) = match self.kind(id) {
            Node::ForStmt { init, compare, update, body } => (init, compare, update, body),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        if !init.is_empty() {
            self.printer.print_ln_s(" {");
            self.printer.indent();
            for &e in &init {
                self.visit(e, arg);
                self.printer.print_ln_s(";");
            }
        }
        if let Some(c) = compare {
            self.printer.print("while ");
            self.visit(c, arg);
        } else {
            self.printer.print("loop ");
        }
        if !update.is_empty() {
            self.printer.print_ln_s(" {");
            self.printer.indent();
        }
        self.encapsulate_if_not_block(body, arg);
        self.printer.print_ln_s("");
        if !update.is_empty() {
            for &e in &update {
                self.visit(e, arg);
                self.printer.print_ln_s(";");
            }
            self.printer.unindent();
            self.printer.print_ln_s(" }");
        }
        if !init.is_empty() {
            self.printer.unindent();
            self.printer.print_ln_s(" }");
        }
    }

    fn visit_try(&mut self, id: NodeId, arg: Arg) {
        let (resources, try_block, catchs, finally_block) = match self.kind(id) {
            Node::TryStmt { resources, try_block, catchs, finally_block } => {
                (resources, try_block, catchs, finally_block)
            }
            _ => unreachable!(),
        };
        // Rust has no try/catch. Run the resource bindings + try body in a scope,
        // then the finally body. Catch clauses (the error path) are dropped.
        self.print_java_comment(id, arg);
        if !resources.is_empty() {
            self.printer.print_ln_s("{");
            self.printer.indent();
            for &r in &resources {
                self.visit(r, arg);
                self.printer.print_ln_s(";");
            }
            self.visit(try_block, arg);
            self.printer.print_ln();
            self.printer.unindent();
            self.printer.print("}");
        } else {
            self.visit(try_block, arg);
        }
        if !catchs.is_empty() {
            self.printer.print(" /* catch clauses omitted */");
        }
        if let Some(f) = finally_block {
            self.printer.print(" ");
            self.visit(f, arg);
        }
    }

    fn visit_lambda(&mut self, id: NodeId, arg: Arg) {
        let (parameters, body, parameters_enclosed) = match self.kind(id) {
            Node::LambdaExpr { parameters, body, parameters_enclosed } => {
                (parameters, body, parameters_enclosed)
            }
            _ => unreachable!(),
        };
        // Rust closure: |params| body  (parameter types are inferred).
        self.print_java_comment(id, arg);
        let _ = parameters_enclosed;
        self.printer.print("|");
        for (i, &p) in parameters.iter().enumerate() {
            let name = self.param_name(p);
            self.printer.print(&name);
            if i + 1 < parameters.len() {
                self.printer.print(", ");
            }
        }
        self.printer.print("| ");
        if let Node::ExpressionStmt { expression } = self.arena.kind(body) {
            let e = *expression;
            self.visit(e, arg);
        } else {
            self.visit(body, arg);
        }
    }

    /// The (snake-cased) name of a Parameter's declarator id.
    fn param_name(&self, p: NodeId) -> String {
        if let Node::Parameter { id, .. } = self.arena.kind(p) {
            if let Node::VariableDeclaratorId { name } = self.arena.kind(*id) {
                return self.to_snake_if_necessary(name);
            }
        }
        String::new()
    }

    fn visit_method_ref(&mut self, id: NodeId, arg: Arg) {
        let (scope, type_arguments, identifier) = match self.kind(id) {
            Node::MethodReferenceExpr { scope, type_arguments, identifier } => {
                (scope, type_arguments, identifier)
            }
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        if let Some(s) = scope {
            self.visit(s, arg);
        }
        self.printer.print("::");
        if !type_arguments.is_empty() {
            self.printer.print("<");
            for (i, &p) in type_arguments.iter().enumerate() {
                self.visit(p, arg);
                if i + 1 < type_arguments.len() {
                    self.printer.print(", ");
                }
            }
            self.printer.print(">");
        }
        if !identifier.is_empty() {
            self.printer.print(&identifier);
        }
    }

    // ---- comments / position ----

    fn annotation_simple_name(&self, name: NodeId) -> String {
        match self.arena.kind(name) {
            Node::NameExpr { name } => name.clone(),
            Node::QualifiedNameExpr { name, .. } => name.clone(),
            _ => String::new(),
        }
    }

    fn is_non_static_field_declaration(&self, n: NodeId) -> bool {
        if matches!(self.arena.kind(n), Node::VariableDeclaratorId { .. }) {
            if let Some(p) = self.arena.parent(n) {
                if let Some(g) = self.arena.parent(p) {
                    if let Node::FieldDeclaration { modifiers, .. } = self.arena.kind(g) {
                        return !modifiers::is_static(*modifiers);
                    }
                }
            }
        }
        false
    }

    fn is_non_static_method_declaration(&self, n: NodeId) -> bool {
        if let Node::MethodDeclaration { modifiers, .. } = self.arena.kind(n) {
            !modifiers::is_static(*modifiers)
        } else {
            false
        }
    }

    fn stop_history_search(&self, n: NodeId) -> bool {
        matches!(
            self.arena.kind(n),
            Node::VariableDeclarator { .. }
                | Node::MethodCallExpr { .. }
                | Node::ArrayAccessExpr { .. }
        ) || self.is_statement(n)
    }

    fn is_statement(&self, n: NodeId) -> bool {
        matches!(
            self.arena.kind(n),
            Node::BlockStmt { .. }
                | Node::ExpressionStmt { .. }
                | Node::ReturnStmt { .. }
                | Node::IfStmt { .. }
                | Node::WhileStmt { .. }
                | Node::DoStmt { .. }
                | Node::ForStmt { .. }
                | Node::ForeachStmt { .. }
                | Node::BreakStmt { .. }
                | Node::ContinueStmt { .. }
                | Node::ThrowStmt { .. }
                | Node::TryStmt { .. }
                | Node::SwitchStmt { .. }
                | Node::SwitchEntryStmt { .. }
                | Node::LabeledStmt { .. }
                | Node::AssertStmt { .. }
                | Node::SynchronizedStmt { .. }
                | Node::EmptyStmt
                | Node::TypeDeclarationStmt { .. }
                | Node::ExplicitConstructorInvocationStmt { .. }
                | Node::CatchClause { .. }
        )
    }

    fn is_float_in_siblings(&self, n: NodeId) -> bool {
        let parent = match self.arena.parent(n) {
            Some(p) => p,
            None => return false,
        };
        if self.stop_history_search(parent) {
            return false;
        }
        for sibling in self.arena.children(parent) {
            if self.id.is_float_node(Some(sibling)) {
                return true;
            }
        }
        false
    }

    fn is_float_in_history(&self, n: Option<NodeId>) -> bool {
        let id = match n {
            Some(x) => x,
            None => return false,
        };
        if self.stop_history_search(id) {
            return false;
        }
        if self.is_float_in_siblings(id) {
            return true;
        }
        let clazz = self.id.get_type(id);
        if clazz.map(IdTracker::is_float_class).unwrap_or(false) {
            true
        } else {
            self.is_float_in_history(self.arena.parent(id))
        }
    }

    fn sort_children_by_begin(&self, parent: NodeId) -> Vec<NodeId> {
        let mut everything = self.arena.children(parent);
        everything.sort_by(|&a, &b| {
            let pa = self.arena.begin(a);
            let pb = self.arena.begin(b);
            (pa.line, pa.column).cmp(&(pb.line, pb.column))
        });
        everything
    }

    fn is_comment(&self, n: NodeId) -> bool {
        matches!(
            self.arena.kind(n),
            Node::LineComment { .. } | Node::BlockComment { .. } | Node::JavadocComment { .. }
        )
    }

    fn print_orphan_comments_before_this_child_node(&mut self, node: NodeId) {
        if self.is_comment(node) {
            return;
        }
        let parent = match self.arena.parent(node) {
            Some(p) => p,
            None => return,
        };
        let everything = self.sort_children_by_begin(parent);
        let mut pos_child: i64 = -1;
        for (i, &e) in everything.iter().enumerate() {
            if e == node {
                pos_child = i as i64;
            }
        }
        if pos_child == -1 {
            // Should not happen given parent links; be lenient.
            return;
        }
        let mut pos_prev: i64 = -1;
        let mut i = pos_child - 1;
        while i >= 0 && pos_prev == -1 {
            if !self.is_comment(everything[i as usize]) {
                pos_prev = i;
            }
            i -= 1;
        }
        for i in (pos_prev + 1)..pos_child {
            let to_print = everything[i as usize];
            if self.is_comment(to_print) {
                self.visit(to_print, None);
            }
        }
    }

    fn print_orphan_comments_ending(&mut self, node: NodeId) {
        let everything = self.sort_children_by_begin(node);
        if everything.is_empty() {
            return;
        }
        let mut comments_at_end = 0usize;
        let mut finding = true;
        while finding && comments_at_end < everything.len() {
            let last = everything[everything.len() - 1 - comments_at_end];
            finding = self.is_comment(last);
            if finding {
                comments_at_end += 1;
            }
        }
        for i in 0..comments_at_end {
            let c = everything[everything.len() - comments_at_end + i];
            self.visit(c, None);
        }
    }
}

/// Convert a Java hex floating-point literal (`0x1.8p3`) to a decimal Rust
/// literal. Returns None if `s` is not a hex float.
fn hex_float_to_decimal(s: &str) -> Option<String> {
    let s = s.trim_end_matches(['f', 'F', 'd', 'D']);
    let lower = s.to_ascii_lowercase();
    let body = lower.strip_prefix("0x")?;
    let (mant, exp) = body.split_once('p')?;
    let exp: i32 = exp.parse().ok()?;
    let (ip, fp) = mant.split_once('.').unwrap_or((mant, ""));
    let mut val = 0f64;
    for c in ip.chars() {
        val = val * 16.0 + c.to_digit(16)? as f64;
    }
    let mut scale = 1.0 / 16.0;
    for c in fp.chars() {
        val += c.to_digit(16)? as f64 * scale;
        scale /= 16.0;
    }
    val *= 2f64.powi(exp);
    Some(format!("{val:e}"))
}

/// If `s` is a Rust keyword, escape it as a raw identifier (`r#s`). A few
/// keywords (`self`/`super`/`crate`/`Self`) cannot be raw, but they are also
/// reserved in Java so never appear as user identifiers.
fn escape_rust_keyword(s: String) -> String {
    const KW: &[&str] = &[
        "as", "break", "const", "continue", "dyn", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "static", "struct", "trait", "true", "type", "unsafe", "use", "where", "while",
        "async", "await", "box", "do", "final", "macro", "override", "priv", "try", "typeof",
        "unsized", "virtual", "yield", "abstract", "become", "gen",
    ];
    if KW.contains(&s.as_str()) {
        format!("r#{s}")
    } else {
        s
    }
}

/// Convert a Java `String.format`/`printf` format string to a Rust `format!`
/// template: `%d`/`%s`/`%.2f`/… → `{}`, `%n` → newline, `%%` → `%`. Literal
/// braces are escaped.
fn java_format_to_rust(fmt: &str) -> String {
    let mut out = String::new();
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => out.push_str("{{"),
            '}' => out.push_str("}}"),
            '%' => {
                if chars.peek() == Some(&'%') {
                    chars.next();
                    out.push('%');
                    continue;
                }
                // Skip flags / width / precision / argument-index.
                while matches!(
                    chars.peek(),
                    Some('0'..='9') | Some('$') | Some('.') | Some(',') | Some('+')
                        | Some('-') | Some(' ') | Some('#') | Some('(')
                ) {
                    chars.next();
                }
                // The conversion character.
                match chars.next() {
                    Some('n') => out.push_str("\\n"),
                    _ => out.push_str("{}"),
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// Map a Java type simple name to its Rust equivalent (collections, boxed
/// primitives). Returns the name unchanged if there is no mapping.
fn map_type_name(name: &str) -> &str {
    // Use the simple name (drop any `pkg.` qualifier) for the mapping.
    let name = name.rsplit('.').next().unwrap_or(name);
    match name {
        "List" | "ArrayList" | "LinkedList" | "Collection" | "AbstractList" | "Vector"
        | "Stack" | "Queue" | "Deque" | "ArrayDeque" | "Iterable" => "Vec",
        "Map" | "HashMap" | "LinkedHashMap" | "TreeMap" | "SortedMap" | "NavigableMap"
        | "AbstractMap" => "std::collections::HashMap",
        "Set" | "HashSet" | "LinkedHashSet" | "TreeSet" | "SortedSet" | "NavigableSet"
        | "AbstractSet" => "std::collections::HashSet",
        "Optional" => "Option",
        "Integer" => "i32",
        "Long" => "i64",
        "Short" => "i16",
        "Byte" => "i8",
        "Double" => "f64",
        "Float" => "f32",
        "Boolean" => "bool",
        "Character" => "char",
        other => other,
    }
}

/// Does this (already-mapped) Rust type name name a collection constructed with
/// `::new()` (no arguments)?
fn is_rust_collection(mapped: &str) -> bool {
    matches!(mapped, "Vec" | "std::collections::HashMap" | "std::collections::HashSet")
}

/// Mirrors `StringUtils.endsWithAny` for a single non-null suffix.
fn ends_with_ignore_none(value: &str, suffix: &str) -> bool {
    value.ends_with(suffix)
}
