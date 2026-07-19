#[derive(Debug, Clone, Default, PartialEq)]
pub struct RustFile {
    pub items: Vec<RustItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RustItem {
    Struct(RustStruct),
    Impl(RustImpl),
    Function(RustFunction),
    Comment(String),
    Raw(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RustStruct {
    pub name: String,
    pub fields: Vec<RustField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RustField {
    pub name: String,
    pub ty: RustType,
    pub is_pub: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RustImpl {
    pub target: String,
    pub associated_consts: Vec<RustAssociatedConst>,
    pub functions: Vec<RustFunction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RustAssociatedConst {
    pub name: String,
    pub ty: RustType,
    pub value: RustExpr,
    pub is_pub: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RustFunction {
    pub name: String,
    pub params: Vec<RustParam>,
    pub return_type: Option<RustType>,
    pub body: Vec<RustStmt>,
    pub is_pub: bool,
    pub receiver: Option<RustReceiver>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustReceiver {
    Shared,
    Mutable,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RustParam {
    pub name: String,
    pub ty: RustType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RustType {
    Unit,
    Bool,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Char,
    String,
    Vec(Box<RustType>),
    Slice(Box<RustType>),
    Option(Box<RustType>),
    Named(String),
    Ref { mutable: bool, inner: Box<RustType> },
    Raw(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RustStmt {
    Let {
        mutable: bool,
        name: String,
        ty: Option<RustType>,
        value: Option<RustExpr>,
    },
    Assign {
        target: RustExpr,
        value: RustExpr,
    },
    Expr(RustExpr),
    Return(Option<RustExpr>),
    If {
        condition: RustExpr,
        then_body: Vec<RustStmt>,
        else_body: Vec<RustStmt>,
    },
    While {
        condition: RustExpr,
        body: Vec<RustStmt>,
    },
    Loop {
        label: Option<String>,
        body: Vec<RustStmt>,
    },
    Break(Option<String>),
    Continue(Option<String>),
    Raw(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RustExpr {
    Literal(String),
    Var(String),
    Field(Box<RustExpr>, String),
    Index(Box<RustExpr>, Box<RustExpr>),
    Binary {
        op: String,
        left: Box<RustExpr>,
        right: Box<RustExpr>,
    },
    Unary {
        op: String,
        expr: Box<RustExpr>,
    },
    If {
        condition: Box<RustExpr>,
        then_expr: Box<RustExpr>,
        else_expr: Box<RustExpr>,
    },
    Call {
        function: Box<RustExpr>,
        args: Vec<RustExpr>,
    },
    MethodCall {
        receiver: Box<RustExpr>,
        method: String,
        args: Vec<RustExpr>,
    },
    StructLit {
        name: String,
        fields: Vec<(String, RustExpr)>,
    },
    Raw(String),
}
