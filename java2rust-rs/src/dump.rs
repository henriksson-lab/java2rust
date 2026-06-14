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
    NonField,
    All,
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
        if first.is_lowercase() {
            camel_to_snake_case(n)
        } else {
            n.to_string()
        }
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
                Filter::All => true,
                Filter::NonField => !matches!(self.arena.kind(member), Node::FieldDeclaration { .. }),
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
                self.printer.print("/**");
                self.printer.print(&content);
                self.printer.print_ln_s("*/");
            }
            ClassOrInterfaceType { .. } => self.visit_class_type(id, arg),
            TypeParameter { name, type_bound } => {
                self.print_java_comment(id, arg);
                self.printer.print(&name);
                if !type_bound.is_empty() {
                    self.printer.print(" extends ");
                    for (i, &c) in type_bound.iter().enumerate() {
                        self.visit(c, arg);
                        if i + 1 < type_bound.len() {
                            self.printer.print(" & ");
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
                self.print_java_comment(id, arg);
                self.printer.print("?");
                if let Some(e) = ext {
                    self.printer.print(" extends ");
                    self.visit(e, arg);
                }
                if let Some(s) = sup {
                    self.printer.print(" super ");
                    self.visit(s, arg);
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
                let mut value = value;
                if !value.contains(['.', 'e', 'E', 'x', 'X']) {
                    value = format!("{value}.0");
                }
                let s = self.remove_plus_and_suffix(value, &["D", "d"]);
                self.printer.print(&s);
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
                // Java String is an owned type; emit an owned Rust String so
                // `String`-typed bindings/returns type-check. (String-concat goes
                // through format! separately and never reaches here.)
                self.printer.print("\"");
                self.printer.print(&value);
                self.printer.print("\".to_string()");
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
                } else if !self.id.is_in_constructor() {
                    self.printer.print("self");
                } else {
                    self.printer.print("let ");
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
            ExplicitConstructorInvocationStmt {
                is_this,
                expr,
                type_args,
                args,
            } => {
                self.print_java_comment(id, arg);
                if is_this {
                    self.print_type_args(&type_args, arg);
                    self.printer.print("this");
                } else {
                    if let Some(e) = expr {
                        self.visit(e, arg);
                        self.printer.print(".");
                    }
                    self.print_type_args(&type_args, arg);
                    self.printer.print("super");
                }
                self.print_arguments(&args, arg);
                self.printer.print(";");
            }
            VariableDeclarationExpr { modifiers: m, typ, vars } => {
                self.print_java_comment(id, arg);
                self.print_modifiers(m);
                self.printer.print(" ");
                for (i, &v) in vars.iter().enumerate() {
                    self.visit(v, Some(typ));
                    if i + 1 < vars.len() {
                        self.printer.print(", ");
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
            SwitchEntryStmt { .. } => self.visit_switch_entry(id, arg),
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
                self.print_java_comment(id, arg);
                self.printer.print(";");
            }
            InitializerDeclaration { is_static, block } => {
                self.print_java_comment(id, arg);
                if is_static {
                    self.printer.print("static ");
                }
                self.visit(block, arg);
            }
            IfStmt { .. } => self.visit_if(id, arg),
            WhileStmt { condition, body } => {
                self.print_java_comment(id, arg);
                self.printer.print("while ");
                self.visit(condition, arg);
                self.printer.print(" ");
                self.visit(body, arg);
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
                self.visit(variable, arg);
                self.printer.print(" in ");
                self.visit(iterable, arg);
                self.printer.print(" ");
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
        // Derive Clone so field values can be cloned out from behind `&self`.
        self.printer.print_ln_s("#[derive(Clone)]");
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
        // methods / constructors / nested / initializers
        self.print_members(&members, arg, Filter::NonField);
        self.print_orphan_comments_ending(id);
        self.printer.unindent();
        self.printer.print_ln_s("}");
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
        self.print_members(members, arg, Filter::NonField);
        self.printer.unindent();
        self.printer.print_ln_s("}");
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
            self.printer.print(".");
        }
        self.printer.print(&name);
        if using_diamond {
            self.printer.print("<>");
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
        let is_initialized_array = init
            .map(|i| {
                matches!(
                    self.arena.kind(i),
                    Node::ArrayInitializerExpr { .. } | Node::ArrayCreationExpr { .. }
                )
            })
            .unwrap_or(false);
        if self.is_type(arg) && !is_initialized_array {
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
            if !is_initialized_array {
                self.printer.print(" = ");
            }
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

    fn get_dimensions(&self, n: NodeId) -> Vec<i32> {
        // Mirrors RustDumpVisitor.getDimensions for ArrayInitializerExpr.
        let mut dimensions = Vec::new();
        let mut cur = Some(n);
        let mut actsize = match self.arena.kind(n) {
            Node::ArrayInitializerExpr { values } => Some(values.len() as i32),
            _ => None,
        };
        while let Some(node) = cur {
            if let Some(sz) = actsize {
                dimensions.push(sz);
            }
            actsize = None;
            let values = match self.arena.kind(node) {
                Node::ArrayInitializerExpr { values } => values.clone(),
                _ => break,
            };
            let first = values[0];
            if matches!(self.arena.kind(first), Node::ArrayInitializerExpr { .. }) {
                let mut size: Option<i32> = None;
                let mut chosen = node;
                for e in &values {
                    if let Node::ArrayInitializerExpr { values: vs } = self.arena.kind(*e) {
                        let l = vs.len() as i32;
                        match size {
                            None => {
                                size = Some(l);
                                chosen = *e;
                            }
                            Some(s) if s < l => {
                                size = Some(l);
                                chosen = *e;
                            }
                            _ => {}
                        }
                    }
                }
                actsize = size;
                cur = Some(chosen);
            } else {
                cur = None;
            }
        }
        dimensions
    }

    fn visit_array_initializer(&mut self, id: NodeId, arg: Arg) {
        let values = match self.kind(id) {
            Node::ArrayInitializerExpr { values } => values,
            _ => unreachable!(),
        };
        let t = if self.is_type(arg) { arg } else { None };
        self.print_java_comment(id, arg);
        if !values.is_empty() {
            if let Some(tn) = t {
                let mut dims = self.get_dimensions(id);
                let mut sb = self.accept_and_cut(tn, arg);
                dims.reverse();
                for i in dims {
                    sb = format!("vec![{sb}; {i}]");
                }
                self.printer.print(": ");
                self.printer.print(&sb);
                self.printer.print(" = ");
            }
            self.printer.print("vec![");
            for &val in &values {
                self.visit(val, None);
                self.printer.print(", ");
            }
            self.printer.print_ln_s("]");
        }
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

    fn get_array_declaration(&self, type_or_default: &str, dims: &[String]) -> String {
        let mut sb = type_or_default.to_string();
        let mut rev = dims.to_vec();
        rev.reverse();
        for s in rev {
            sb = format!("[{sb}; {s}]");
        }
        sb
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
        if !dimensions.is_empty() {
            let mut ty = self.accept_and_cut(typ, arg);
            let default = self.default_value(&ty);
            if default == "None" {
                ty = format!("Option<{ty}>");
            }
            let dims: Vec<String> = dimensions
                .iter()
                .map(|&e| self.accept_and_cut(e, arg))
                .collect();
            self.printer.print(": ");
            let decl = self.get_array_declaration(&ty, &dims);
            self.printer.print(&decl);
            self.printer.print(" = ");
            let decl2 = self.get_array_declaration(&default, &dims);
            self.printer.print(&decl2);
        } else {
            self.printer.print(" ");
            if let Some(init) = initializer {
                self.visit(init, Some(typ));
            }
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

    fn replace_length_at_end(&self, field_access: &str) -> String {
        if field_access == "length" {
            "len()".to_string()
        } else {
            field_access.to_string()
        }
    }

    fn visit_field_access(&mut self, id: NodeId, arg: Arg) {
        let (scope, field) = match self.kind(id) {
            Node::FieldAccessExpr { scope, field, .. } => (scope, field),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        let mark = self.printer.push();
        self.visit(scope, arg);
        let scope_str = self.printer.get_mark(mark);
        self.printer.drop();
        let stripped = scope_str.trim_end_matches(' ');
        let i = stripped
            .rfind(['\n', '\t', ' ', '.'])
            .map(|x| x as i64)
            .unwrap_or(-1);
        let accessed: String = if i <= 0 {
            scope_str.clone()
        } else {
            scope_str[(i as usize + 1)..].to_string()
        };
        let chars: Vec<char> = accessed.chars().collect();
        if !chars.is_empty()
            && chars[0].is_uppercase()
            && chars.len() > 1
            && chars[1].is_lowercase()
        {
            self.printer.print("::");
        } else {
            self.printer.print(".");
        }
        let f = self.replace_length_at_end(&field);
        self.printer.print(&f);
    }

    fn visit_method_call(&mut self, id: NodeId, arg: Arg) {
        let (scope, type_args, name, args) = match self.kind(id) {
            Node::MethodCallExpr { scope, type_args, name, args } => (scope, type_args, name, args),
            _ => unreachable!(),
        };
        if self.try_emit_print_macro(scope, &name, &args, arg) {
            return;
        }
        self.print_java_comment(id, arg);
        if let Some(s) = scope {
            self.visit(s, arg);
            if first_char_upper(self.first_char_java(s)) {
                self.printer.print("::");
            } else {
                self.printer.print(".");
            }
        }
        self.print_type_args(&type_args, arg);
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
        self.print_type_args(&type_args, arg);
        if !type_args.is_empty() {
            self.printer.print(" ");
        }
        self.visit(typ, arg);
        self.printer.print("::new");
        self.print_arguments(&args, arg);
        if let Some(body) = anonymous_body {
            self.printer.print_ln_s(" {");
            self.printer.indent();
            self.print_members(&body, arg, Filter::All);
            self.printer.unindent();
            self.printer.print("}");
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
            UnaryOp::Inverse => self.printer.print("~"),
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
        if !throws.is_empty() {
            self.printer.print(" throws ");
            for (i, &r) in throws.iter().enumerate() {
                self.visit(r, arg);
                if i + 1 < throws.len() {
                    self.printer.print(", ");
                }
            }
        }
        self.printer.print(" ");
        self.visit(block, arg);
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
        self.print_modifiers(modifiers_v);
        self.printer.print("fn ");
        if is_default {
            self.printer.print("default ");
        }
        self.print_type_parameters(&type_parameters, arg);
        if !type_parameters.is_empty() {
            self.printer.print(" ");
        }
        let raw_type = self.accept_and_cut(typ, arg);
        let ret_nullable = self.decl_nullable(id) && raw_type.trim() != "void";
        let type_string = if ret_nullable {
            format!("Option<{}>", raw_type.trim())
        } else {
            raw_type.clone()
        };
        self.printer.print(" ");
        let snake = self.to_snake_if_necessary(&name);
        self.printer.print(&snake);
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
        let (typ, vid) = match self.kind(id) {
            Node::Parameter { typ, id: vid, .. } => (typ, vid),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        self.printer.print(" ");
        self.visit(vid, arg);
        self.printer.print(": ");
        let nullable = self.decl_nullable(vid);
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
        self.printer.print("match ");
        self.visit(selector, arg);
        self.printer.print_ln_s(" {");
        self.printer.indent();
        for &e in &entries {
            self.visit(e, arg);
        }
        self.printer.unindent();
        self.printer.print("}");
    }

    fn visit_switch_entry(&mut self, id: NodeId, arg: Arg) {
        let (label, stmts) = match self.kind(id) {
            Node::SwitchEntryStmt { label, stmts } => (label, stmts),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        if let Some(l) = label {
            self.printer.print("  ");
            self.visit(l, arg);
            self.printer.print(" => ");
        } else {
            self.printer.print("_ => ");
        }
        self.printer.print_ln();
        self.printer.indent();
        if !stmts.is_empty() {
            self.printer.print_ln_s(" {");
            self.printer.indent();
            for &s in &stmts {
                self.visit(s, arg);
                self.printer.print_ln();
            }
            self.printer.unindent();
            self.printer.print_ln_s("}");
        }
        self.printer.unindent();
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
        if !implements.is_empty() {
            self.printer.print(" implements ");
            for (i, &c) in implements.iter().enumerate() {
                self.visit(c, arg);
                if i + 1 < implements.len() {
                    self.printer.print(", ");
                }
            }
        }
        self.printer.print_ln_s(" {");
        self.printer.indent();
        if !entries.is_empty() {
            self.printer.print_ln();
            for (i, &e) in entries.iter().enumerate() {
                self.visit(e, arg);
                if i + 1 < entries.len() {
                    self.printer.print(", ");
                }
            }
        }
        if !members.is_empty() {
            self.printer.print_ln_s(";");
            self.print_members(&members, arg, Filter::All);
        } else if !entries.is_empty() {
            self.printer.print_ln();
        }
        self.printer.unindent();
        self.printer.print("}");
    }

    fn visit_enum_constant(&mut self, id: NodeId, arg: Arg) {
        let (name, args, class_body) = match self.kind(id) {
            Node::EnumConstantDeclaration { name, args, class_body } => (name, args, class_body),
            _ => unreachable!(),
        };
        self.print_java_comment(id, arg);
        self.printer.print(&name);
        // JavaParser's EnumConstantDeclaration.getArgs() is always non-null, so
        // printArguments is always invoked (yielding "()" with no arguments).
        self.print_arguments(&args, arg);
        if !class_body.is_empty() {
            self.printer.print_ln_s(" {");
            self.printer.indent();
            self.print_members(&class_body, arg, Filter::All);
            self.printer.unindent();
            self.printer.print_ln_s("}");
        }
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
        self.print_java_comment(id, arg);
        if parameters_enclosed {
            self.printer.print("(");
        }
        for (i, &p) in parameters.iter().enumerate() {
            self.visit(p, arg);
            if i + 1 < parameters.len() {
                self.printer.print(", ");
            }
        }
        if parameters_enclosed {
            self.printer.print(")");
        }
        self.printer.print(" -> ");
        if let Node::ExpressionStmt { expression } = self.arena.kind(body) {
            let e = *expression;
            self.visit(e, arg);
        } else {
            self.visit(body, arg);
        }
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

    /// First character of the JavaParser `toString()` of a node (leftmost token).
    fn first_char_java(&self, id: NodeId) -> char {
        match self.arena.kind(id) {
            Node::NameExpr { name } => name.chars().next().unwrap_or(' '),
            Node::QualifiedNameExpr { qualifier, .. } => self.first_char_java(*qualifier),
            Node::FieldAccessExpr { scope, .. } => self.first_char_java(*scope),
            Node::MethodCallExpr { scope, name, .. } => match scope {
                Some(s) => self.first_char_java(*s),
                None => name.chars().next().unwrap_or(' '),
            },
            Node::ArrayAccessExpr { name, .. } => self.first_char_java(*name),
            Node::ThisExpr { .. } => 't',
            Node::SuperExpr { .. } => 's',
            Node::EnclosedExpr { .. } => '(',
            Node::CastExpr { .. } => '(',
            Node::ObjectCreationExpr { .. } => 'n',
            Node::ClassExpr { typ } => self.first_char_java(*typ),
            Node::ClassOrInterfaceType { scope, name, .. } => match scope {
                Some(s) => self.first_char_java(*s),
                None => name.chars().next().unwrap_or(' '),
            },
            Node::ReferenceType { typ, .. } => self.first_char_java(*typ),
            Node::StringLiteralExpr { .. } => '"',
            Node::IntegerLiteralExpr { value }
            | Node::LongLiteralExpr { value }
            | Node::DoubleLiteralExpr { value } => value.chars().next().unwrap_or(' '),
            _ => {
                // Fallback: render and take first char.
                ' '
            }
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

fn first_char_upper(c: char) -> bool {
    c.is_uppercase()
}

/// Mirrors `StringUtils.endsWithAny` for a single non-null suffix.
fn ends_with_ignore_none(value: &str, suffix: &str) -> bool {
    value.ends_with(suffix)
}
