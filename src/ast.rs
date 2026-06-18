//! Typed AST mirroring the JavaParser 2.5.1 node types used by java2rust.
//!
//! Stored in an arena (`Arena`) so nodes can be referenced by a stable
//! [`NodeId`] — this gives us JavaParser's identity-keyed maps (`IdentityHashMap`)
//! and parent navigation (`getParentNode`) without `Rc`/`RefCell` gymnastics.

/// 1-based line / 1-based column, matching JavaParser positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pos {
    pub line: i32,
    pub column: i32,
}

/// Stable handle into the [`Arena`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

/// A modelled `java.lang.Class` value, as used by `IdTracker`/`TypeTracker`.
///
/// Only the distinctions the converter actually makes are represented:
/// primitive `*.TYPE`, boxed wrapper classes, `String`, and any other class
/// that resolved by name (carrying nothing but "not primitive / not numeric").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JClass {
    // primitive .TYPE values
    BooleanType,
    ByteType,
    ShortType,
    IntType,
    LongType,
    FloatType,
    DoubleType,
    CharType,
    VoidType,
    // boxed wrapper classes
    BooleanClass,
    ByteClass,
    ShortClass,
    IntegerClass,
    LongClass,
    FloatClass,
    DoubleClass,
    CharacterClass,
    // java.lang.String
    StringClass,
    /// Some other resolved class (object reference, not numeric/boolean/string).
    Other,
}

impl JClass {
    /// Mirrors `Class.isPrimitive()`.
    pub fn is_primitive(self) -> bool {
        matches!(
            self,
            JClass::BooleanType
                | JClass::ByteType
                | JClass::ShortType
                | JClass::IntType
                | JClass::LongType
                | JClass::FloatType
                | JClass::DoubleType
                | JClass::CharType
                | JClass::VoidType
        )
    }

    /// Mirrors `Class.getTypeName()` for the values where the converter inspects it.
    pub fn type_name(self) -> &'static str {
        match self {
            JClass::BooleanType => "boolean",
            JClass::ByteType => "byte",
            JClass::ShortType => "short",
            JClass::IntType => "int",
            JClass::LongType => "long",
            JClass::FloatType => "float",
            JClass::DoubleType => "double",
            JClass::CharType => "char",
            JClass::VoidType => "void",
            JClass::BooleanClass => "java.lang.Boolean",
            JClass::ByteClass => "java.lang.Byte",
            JClass::ShortClass => "java.lang.Short",
            JClass::IntegerClass => "java.lang.Integer",
            JClass::LongClass => "java.lang.Long",
            JClass::FloatClass => "java.lang.Float",
            JClass::DoubleClass => "java.lang.Double",
            JClass::CharacterClass => "java.lang.Character",
            JClass::StringClass => "java.lang.String",
            JClass::Other => "java.lang.Object",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveKind {
    Boolean,
    Char,
    Byte,
    Short,
    Int,
    Long,
    Float,
    Double,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Or,
    And,
    BinOr,
    BinAnd,
    Xor,
    Equals,
    NotEquals,
    Less,
    Greater,
    LessEquals,
    GreaterEquals,
    LShift,
    RSignedShift,
    RUnsignedShift,
    Plus,
    Minus,
    Times,
    Divide,
    Remainder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,
    And,
    Or,
    Xor,
    Plus,
    Minus,
    Rem,
    Slash,
    Star,
    LShift,
    RSignedShift,
    RUnsignedShift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Positive,
    Negative,
    Inverse,
    Not,
    PreIncrement,
    PreDecrement,
    PosIncrement,
    PosDecrement,
}

/// The node payload. Children are referenced by [`NodeId`]; scalars are inline.
#[derive(Debug, Clone)]
pub enum Node {
    // ---- top level ----
    CompilationUnit {
        package: Option<NodeId>,
        imports: Vec<NodeId>,
        types: Vec<NodeId>,
    },
    PackageDeclaration {
        name: NodeId,
    },
    ImportDeclaration {
        name: NodeId,
        is_static: bool,
        is_asterisk: bool,
    },
    TypeParameter {
        name: String,
        type_bound: Vec<NodeId>,
    },

    // ---- body declarations ----
    ClassOrInterfaceDeclaration {
        modifiers: i32,
        is_interface: bool,
        name: String,
        type_parameters: Vec<NodeId>,
        extends: Vec<NodeId>,
        implements: Vec<NodeId>,
        members: Vec<NodeId>,
    },
    EnumDeclaration {
        modifiers: i32,
        name: String,
        implements: Vec<NodeId>,
        entries: Vec<NodeId>,
        members: Vec<NodeId>,
    },
    EnumConstantDeclaration {
        name: String,
        args: Vec<NodeId>,
        class_body: Vec<NodeId>,
    },
    EmptyTypeDeclaration,
    FieldDeclaration {
        modifiers: i32,
        typ: NodeId,
        variables: Vec<NodeId>,
    },
    MethodDeclaration {
        modifiers: i32,
        typ: NodeId,
        name: String,
        type_parameters: Vec<NodeId>,
        parameters: Vec<NodeId>,
        throws: Vec<NodeId>,
        body: Option<NodeId>,
        array_count: i32,
        is_default: bool,
        annotations: Vec<NodeId>,
    },
    ConstructorDeclaration {
        modifiers: i32,
        name: String,
        type_parameters: Vec<NodeId>,
        parameters: Vec<NodeId>,
        throws: Vec<NodeId>,
        block: NodeId,
    },
    Parameter {
        modifiers: i32,
        typ: Option<NodeId>,
        id: NodeId,
        is_var_args: bool,
    },
    MultiTypeParameter {
        modifiers: i32,
        typ: NodeId,
        id: NodeId,
    },
    VariableDeclarator {
        id: NodeId,
        init: Option<NodeId>,
        /// C-style trailing array dimensions (`String tokens[]`), which attach to
        /// the declarator rather than the shared type. Wraps the type in `Vec<>`.
        array_count: i32,
    },
    VariableDeclaratorId {
        name: String,
    },
    InitializerDeclaration {
        is_static: bool,
        block: NodeId,
    },
    EmptyMemberDeclaration,

    // ---- comments ----
    LineComment {
        content: String,
    },
    BlockComment {
        content: String,
    },
    JavadocComment {
        content: String,
    },

    // ---- types ----
    ClassOrInterfaceType {
        scope: Option<NodeId>,
        name: String,
        type_args: Vec<NodeId>,
        using_diamond: bool,
    },
    PrimitiveType {
        kind: PrimitiveKind,
    },
    ReferenceType {
        typ: NodeId,
        array_count: i32,
    },
    VoidType,
    WildcardType {
        ext: Option<NodeId>,
        sup: Option<NodeId>,
    },
    UnknownType,
    IntersectionType {
        elements: Vec<NodeId>,
    },
    UnionType {
        elements: Vec<NodeId>,
    },

    // ---- expressions ----
    NameExpr {
        name: String,
    },
    QualifiedNameExpr {
        qualifier: NodeId,
        name: String,
    },
    IntegerLiteralExpr {
        value: String,
    },
    LongLiteralExpr {
        value: String,
    },
    DoubleLiteralExpr {
        value: String,
    },
    IntegerLiteralMinValueExpr {
        value: String,
    },
    LongLiteralMinValueExpr {
        value: String,
    },
    StringLiteralExpr {
        value: String,
    },
    CharLiteralExpr {
        value: String,
    },
    BooleanLiteralExpr {
        value: bool,
    },
    NullLiteralExpr,
    BinaryExpr {
        left: NodeId,
        op: BinaryOp,
        right: NodeId,
    },
    UnaryExpr {
        expr: NodeId,
        op: UnaryOp,
    },
    AssignExpr {
        target: NodeId,
        op: AssignOp,
        value: NodeId,
    },
    MethodCallExpr {
        scope: Option<NodeId>,
        type_args: Vec<NodeId>,
        name: String,
        args: Vec<NodeId>,
    },
    FieldAccessExpr {
        scope: NodeId,
        type_args: Vec<NodeId>,
        field: String,
    },
    ObjectCreationExpr {
        scope: Option<NodeId>,
        typ: NodeId,
        type_args: Vec<NodeId>,
        args: Vec<NodeId>,
        anonymous_body: Option<Vec<NodeId>>,
    },
    ArrayAccessExpr {
        name: NodeId,
        index: NodeId,
    },
    ArrayCreationExpr {
        typ: NodeId,
        type_args: Vec<NodeId>,
        array_count: i32,
        dimensions: Vec<NodeId>,
        initializer: Option<NodeId>,
    },
    ArrayInitializerExpr {
        values: Vec<NodeId>,
    },
    CastExpr {
        typ: NodeId,
        expr: NodeId,
    },
    ClassExpr {
        typ: NodeId,
    },
    ConditionalExpr {
        condition: NodeId,
        then_expr: NodeId,
        else_expr: NodeId,
    },
    EnclosedExpr {
        inner: Option<NodeId>,
    },
    InstanceOfExpr {
        expr: NodeId,
        typ: NodeId,
    },
    ThisExpr {
        class_expr: Option<NodeId>,
    },
    SuperExpr {
        class_expr: Option<NodeId>,
    },
    VariableDeclarationExpr {
        modifiers: i32,
        typ: NodeId,
        vars: Vec<NodeId>,
    },
    LambdaExpr {
        parameters: Vec<NodeId>,
        body: NodeId,
        parameters_enclosed: bool,
    },
    MethodReferenceExpr {
        scope: Option<NodeId>,
        type_arguments: Vec<NodeId>,
        identifier: String,
    },
    TypeExpr {
        typ: Option<NodeId>,
    },
    /// MarkerAnnotationExpr / SingleMemberAnnotationExpr / NormalAnnotationExpr.
    AnnotationExpr {
        name: NodeId,
    },
    MemberValuePair {
        name: String,
        value: NodeId,
    },

    // ---- statements ----
    BlockStmt {
        stmts: Vec<NodeId>,
    },
    ExpressionStmt {
        expression: NodeId,
    },
    ReturnStmt {
        expr: Option<NodeId>,
    },
    IfStmt {
        condition: NodeId,
        then_stmt: NodeId,
        else_stmt: Option<NodeId>,
    },
    WhileStmt {
        condition: NodeId,
        body: NodeId,
    },
    DoStmt {
        body: NodeId,
        condition: NodeId,
    },
    ForStmt {
        init: Vec<NodeId>,
        compare: Option<NodeId>,
        update: Vec<NodeId>,
        body: NodeId,
    },
    ForeachStmt {
        variable: NodeId,
        iterable: NodeId,
        body: NodeId,
    },
    BreakStmt {
        id: Option<String>,
    },
    ContinueStmt {
        id: Option<String>,
    },
    ThrowStmt {
        expr: NodeId,
    },
    TryStmt {
        resources: Vec<NodeId>,
        try_block: NodeId,
        catchs: Vec<NodeId>,
        finally_block: Option<NodeId>,
    },
    CatchClause {
        param: NodeId,
        catch_block: NodeId,
    },
    SwitchStmt {
        selector: NodeId,
        entries: Vec<NodeId>,
    },
    SwitchEntryStmt {
        label: Option<NodeId>,
        stmts: Vec<NodeId>,
    },
    LabeledStmt {
        label: String,
        stmt: NodeId,
    },
    AssertStmt {
        check: NodeId,
        message: Option<NodeId>,
    },
    SynchronizedStmt {
        expr: NodeId,
        block: NodeId,
    },
    EmptyStmt,
    TypeDeclarationStmt {
        type_declaration: NodeId,
    },
    ExplicitConstructorInvocationStmt {
        is_this: bool,
        expr: Option<NodeId>,
        type_args: Vec<NodeId>,
        args: Vec<NodeId>,
    },
}

#[derive(Debug, Clone)]
pub struct NodeData {
    pub kind: Node,
    pub parent: Option<NodeId>,
    pub comment: Option<NodeId>,
    pub orphan_comments: Vec<NodeId>,
    pub begin: Pos,
    pub end: Pos,
}

#[derive(Debug, Default)]
pub struct Arena {
    pub nodes: Vec<NodeData>,
    /// Root compilation unit, set once parsing finishes.
    pub root: Option<NodeId>,
}

impl Arena {
    pub fn new() -> Self {
        Arena::default()
    }

    pub fn alloc(&mut self, kind: Node, begin: Pos, end: Pos) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(NodeData {
            kind,
            parent: None,
            comment: None,
            orphan_comments: Vec::new(),
            begin,
            end,
        });
        id
    }

    pub fn data(&self, id: NodeId) -> &NodeData {
        &self.nodes[id.0 as usize]
    }

    pub fn data_mut(&mut self, id: NodeId) -> &mut NodeData {
        &mut self.nodes[id.0 as usize]
    }

    pub fn kind(&self, id: NodeId) -> &Node {
        &self.nodes[id.0 as usize].kind
    }

    /// Number of allocated nodes — every node has id `0..node_count()`, so this
    /// allows a flat scan of all nodes without recursion.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id.0 as usize].parent
    }

    pub fn comment(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id.0 as usize].comment
    }

    pub fn begin(&self, id: NodeId) -> Pos {
        self.nodes[id.0 as usize].begin
    }

    pub fn end(&self, id: NodeId) -> Pos {
        self.nodes[id.0 as usize].end
    }

    /// Child AST nodes in JavaParser insertion order, with the attached comment
    /// appended (JavaParser sets the comment as a child too). Mirrors
    /// `Node.getChildrenNodes()`.
    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        use Node::*;
        let mut out: Vec<NodeId> = Vec::new();
        let push = |o: Option<NodeId>, out: &mut Vec<NodeId>| {
            if let Some(x) = o {
                out.push(x);
            }
        };
        match self.kind(id) {
            CompilationUnit {
                package,
                imports,
                types,
            } => {
                push(*package, &mut out);
                out.extend(imports.iter().copied());
                out.extend(types.iter().copied());
            }
            PackageDeclaration { name } => out.push(*name),
            ImportDeclaration { name, .. } => out.push(*name),
            TypeParameter { type_bound, .. } => out.extend(type_bound.iter().copied()),
            ClassOrInterfaceDeclaration {
                type_parameters,
                extends,
                implements,
                members,
                ..
            } => {
                out.extend(type_parameters.iter().copied());
                out.extend(extends.iter().copied());
                out.extend(implements.iter().copied());
                out.extend(members.iter().copied());
            }
            EnumDeclaration {
                implements,
                entries,
                members,
                ..
            } => {
                out.extend(implements.iter().copied());
                out.extend(entries.iter().copied());
                out.extend(members.iter().copied());
            }
            EnumConstantDeclaration {
                args, class_body, ..
            } => {
                out.extend(args.iter().copied());
                out.extend(class_body.iter().copied());
            }
            FieldDeclaration {
                typ, variables, ..
            } => {
                out.push(*typ);
                out.extend(variables.iter().copied());
            }
            MethodDeclaration {
                typ,
                type_parameters,
                parameters,
                throws,
                body,
                annotations,
                ..
            } => {
                out.extend(annotations.iter().copied());
                out.extend(type_parameters.iter().copied());
                out.push(*typ);
                out.extend(parameters.iter().copied());
                out.extend(throws.iter().copied());
                push(*body, &mut out);
            }
            ConstructorDeclaration {
                type_parameters,
                parameters,
                throws,
                block,
                ..
            } => {
                out.extend(type_parameters.iter().copied());
                out.extend(parameters.iter().copied());
                out.extend(throws.iter().copied());
                out.push(*block);
            }
            Parameter { typ, id: vid, .. } => {
                push(*typ, &mut out);
                out.push(*vid);
            }
            MultiTypeParameter { typ, id: vid, .. } => {
                out.push(*typ);
                out.push(*vid);
            }
            VariableDeclarator { id: vid, init, .. } => {
                out.push(*vid);
                push(*init, &mut out);
            }
            InitializerDeclaration { block, .. } => out.push(*block),
            ClassOrInterfaceType {
                scope, type_args, ..
            } => {
                push(*scope, &mut out);
                out.extend(type_args.iter().copied());
            }
            ReferenceType { typ, .. } => out.push(*typ),
            WildcardType { ext, sup } => {
                push(*ext, &mut out);
                push(*sup, &mut out);
            }
            IntersectionType { elements } | UnionType { elements } => {
                out.extend(elements.iter().copied())
            }
            QualifiedNameExpr { qualifier, .. } => out.push(*qualifier),
            BinaryExpr { left, right, .. } => {
                out.push(*left);
                out.push(*right);
            }
            UnaryExpr { expr, .. } => out.push(*expr),
            AssignExpr { target, value, .. } => {
                out.push(*target);
                out.push(*value);
            }
            MethodCallExpr {
                scope,
                type_args,
                args,
                ..
            } => {
                push(*scope, &mut out);
                out.extend(type_args.iter().copied());
                out.extend(args.iter().copied());
            }
            FieldAccessExpr {
                scope, type_args, ..
            } => {
                out.push(*scope);
                out.extend(type_args.iter().copied());
            }
            ObjectCreationExpr {
                scope,
                typ,
                type_args,
                args,
                anonymous_body,
            } => {
                push(*scope, &mut out);
                out.push(*typ);
                out.extend(type_args.iter().copied());
                out.extend(args.iter().copied());
                if let Some(b) = anonymous_body {
                    out.extend(b.iter().copied());
                }
            }
            ArrayAccessExpr { name, index } => {
                out.push(*name);
                out.push(*index);
            }
            ArrayCreationExpr {
                typ,
                type_args,
                dimensions,
                initializer,
                ..
            } => {
                out.push(*typ);
                out.extend(type_args.iter().copied());
                out.extend(dimensions.iter().copied());
                push(*initializer, &mut out);
            }
            ArrayInitializerExpr { values } => out.extend(values.iter().copied()),
            CastExpr { typ, expr } => {
                out.push(*typ);
                out.push(*expr);
            }
            ClassExpr { typ } => out.push(*typ),
            ConditionalExpr {
                condition,
                then_expr,
                else_expr,
            } => {
                out.push(*condition);
                out.push(*then_expr);
                out.push(*else_expr);
            }
            EnclosedExpr { inner } => push(*inner, &mut out),
            InstanceOfExpr { expr, typ } => {
                out.push(*expr);
                out.push(*typ);
            }
            ThisExpr { class_expr } | SuperExpr { class_expr } => push(*class_expr, &mut out),
            VariableDeclarationExpr { typ, vars, .. } => {
                out.push(*typ);
                out.extend(vars.iter().copied());
            }
            LambdaExpr {
                parameters, body, ..
            } => {
                out.extend(parameters.iter().copied());
                out.push(*body);
            }
            MethodReferenceExpr {
                scope,
                type_arguments,
                ..
            } => {
                push(*scope, &mut out);
                out.extend(type_arguments.iter().copied());
            }
            TypeExpr { typ } => push(*typ, &mut out),
            AnnotationExpr { name } => out.push(*name),
            MemberValuePair { value, .. } => out.push(*value),
            BlockStmt { stmts } => out.extend(stmts.iter().copied()),
            ExpressionStmt { expression } => out.push(*expression),
            ReturnStmt { expr } => push(*expr, &mut out),
            IfStmt {
                condition,
                then_stmt,
                else_stmt,
            } => {
                out.push(*condition);
                out.push(*then_stmt);
                push(*else_stmt, &mut out);
            }
            WhileStmt { condition, body } => {
                out.push(*condition);
                out.push(*body);
            }
            DoStmt { body, condition } => {
                out.push(*body);
                out.push(*condition);
            }
            ForStmt {
                init,
                compare,
                update,
                body,
            } => {
                out.extend(init.iter().copied());
                push(*compare, &mut out);
                out.extend(update.iter().copied());
                out.push(*body);
            }
            ForeachStmt {
                variable,
                iterable,
                body,
            } => {
                out.push(*variable);
                out.push(*iterable);
                out.push(*body);
            }
            ThrowStmt { expr } => out.push(*expr),
            TryStmt {
                resources,
                try_block,
                catchs,
                finally_block,
            } => {
                out.extend(resources.iter().copied());
                out.push(*try_block);
                out.extend(catchs.iter().copied());
                push(*finally_block, &mut out);
            }
            CatchClause { param, catch_block } => {
                out.push(*param);
                out.push(*catch_block);
            }
            SwitchStmt { selector, entries } => {
                out.push(*selector);
                out.extend(entries.iter().copied());
            }
            SwitchEntryStmt { label, stmts } => {
                push(*label, &mut out);
                out.extend(stmts.iter().copied());
            }
            LabeledStmt { stmt, .. } => out.push(*stmt),
            AssertStmt { check, message } => {
                out.push(*check);
                push(*message, &mut out);
            }
            SynchronizedStmt { expr, block } => {
                out.push(*expr);
                out.push(*block);
            }
            TypeDeclarationStmt { type_declaration } => out.push(*type_declaration),
            ExplicitConstructorInvocationStmt {
                expr,
                type_args,
                args,
                ..
            } => {
                push(*expr, &mut out);
                out.extend(type_args.iter().copied());
                out.extend(args.iter().copied());
            }
            // leaf nodes
            EmptyTypeDeclaration
            | EmptyMemberDeclaration
            | LineComment { .. }
            | BlockComment { .. }
            | JavadocComment { .. }
            | PrimitiveType { .. }
            | VoidType
            | UnknownType
            | NameExpr { .. }
            | VariableDeclaratorId { .. }
            | IntegerLiteralExpr { .. }
            | LongLiteralExpr { .. }
            | DoubleLiteralExpr { .. }
            | IntegerLiteralMinValueExpr { .. }
            | LongLiteralMinValueExpr { .. }
            | StringLiteralExpr { .. }
            | CharLiteralExpr { .. }
            | BooleanLiteralExpr { .. }
            | NullLiteralExpr
            | BreakStmt { .. }
            | ContinueStmt { .. }
            | EmptyStmt => {}
        }
        // A node's own comment is NOT part of getChildrenNodes(), but its
        // orphan comments are (they carry this node as their parent).
        out.extend(self.nodes[id.0 as usize].orphan_comments.iter().copied());
        out
    }
}
