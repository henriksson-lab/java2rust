use crate::diagnostics::Span;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Program {
    pub package: Option<String>,
    pub imports: Vec<ImportDecl>,
    pub types: Vec<TypeDecl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportDecl {
    pub path: String,
    pub is_static: bool,
    pub is_wildcard: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeDecl {
    pub kind: TypeKind,
    pub name: String,
    pub modifiers: Vec<Modifier>,
    pub extends: Option<String>,
    pub implements: Vec<String>,
    pub members: Vec<Member>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Class,
    Interface,
    Enum,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Member {
    Field(FieldDecl),
    Method(MethodDecl),
    Constructor(ConstructorDecl),
    Type(TypeDecl),
    Unsupported { reason: String, span: Span },
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    pub name: String,
    pub ty: JavaType,
    pub modifiers: Vec<Modifier>,
    pub initializer: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MethodDecl {
    pub name: String,
    pub rust_name: Option<String>,
    pub return_type: JavaType,
    pub throws: Vec<JavaType>,
    pub params: Vec<Param>,
    pub modifiers: Vec<Modifier>,
    pub body: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstructorDecl {
    pub name: String,
    pub rust_name: Option<String>,
    pub params: Vec<Param>,
    pub throws: Vec<JavaType>,
    pub modifiers: Vec<Modifier>,
    pub body: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: JavaType,
    pub is_varargs: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modifier {
    Public,
    Protected,
    Private,
    Static,
    Final,
    Abstract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JavaType {
    Void,
    Primitive(PrimitiveType),
    String,
    Array(Box<JavaType>),
    Named { name: String, args: Vec<JavaType> },
    Wildcard(Option<WildcardBound>),
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WildcardBound {
    Extends(Box<JavaType>),
    Super(Box<JavaType>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    Boolean,
    Byte,
    Short,
    Char,
    Int,
    Long,
    Float,
    Double,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    VarDecl {
        ty: JavaType,
        name: String,
        initializer: Option<Expr>,
    },
    Assign {
        target: Expr,
        value: Expr,
    },
    Expr(Expr),
    Return(Option<Expr>),
    If {
        condition: Expr,
        then_block: Block,
        else_block: Option<Block>,
    },
    While {
        condition: Expr,
        body: Block,
    },
    For {
        init: Vec<Stmt>,
        condition: Option<Expr>,
        update: Vec<Expr>,
        body: Block,
        label: Option<String>,
    },
    EnhancedFor {
        var_type: JavaType,
        var_name: String,
        iterable: Expr,
        body: Block,
    },
    Switch {
        selector: Expr,
        cases: Vec<SwitchCase>,
    },
    Try {
        body: Block,
        catches: Vec<CatchClause>,
        finally_block: Option<Block>,
    },
    Throw(Expr),
    LocalType(TypeDecl),
    ConstructorInvocation {
        kind: ConstructorInvocationKind,
        args: Vec<Expr>,
        resolved: Option<ResolvedConstructor>,
    },
    Break(Option<String>),
    Continue(Option<String>),
    Block(Block),
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    pub labels: Vec<SwitchLabel>,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SwitchLabel {
    Expr(Expr),
    Default,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatchClause {
    pub types: Vec<JavaType>,
    pub name: String,
    pub body: Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructorInvocationKind {
    This,
    Super,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConstructor {
    pub declaring_type: String,
    pub rust_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Var(String),
    This,
    Super,
    FieldAccess(Box<Expr>, String),
    ArrayAccess(Box<Expr>, Box<Expr>),
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Ternary {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
    },
    Call {
        receiver: Option<Box<Expr>>,
        name: String,
        args: Vec<Expr>,
        resolved: Option<ResolvedCall>,
    },
    New {
        ty: JavaType,
        args: Vec<Expr>,
        body: Option<Vec<Member>>,
        resolved: Option<ResolvedConstructor>,
    },
    Cast {
        ty: JavaType,
        expr: Box<Expr>,
    },
    InstanceOf {
        expr: Box<Expr>,
        ty: JavaType,
    },
    Lambda {
        params: Vec<LambdaParam>,
        body: LambdaBody,
    },
    MethodReference {
        target: String,
        method: String,
    },
    ClassLiteral(JavaType),
    Raw(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct LambdaParam {
    pub name: String,
    pub ty: Option<JavaType>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LambdaBody {
    Expr(Box<Expr>),
    Block(Block),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCall {
    pub declaring_type: String,
    pub rust_name: String,
    pub kind: CallKind,
    pub return_type: JavaType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallKind {
    Instance,
    Static,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Null,
    Bool(bool),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(String),
    Char(char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
    PreInc,
    PreDec,
    PostInc,
    PostDec,
}
