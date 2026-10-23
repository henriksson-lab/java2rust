use crate::diagnostics::Span;

use crate::binding_id::BindingId;

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
    /// Original Java runtime identity, preserved across emitted-name and
    /// compilation-unit merge rewrites. Synthetic/non-named types may lack it.
    pub source_names: Option<SourceTypeNames>,
    pub type_params: Vec<String>,
    pub type_param_bounds: Vec<Option<JavaType>>,
    pub modifiers: Vec<Modifier>,
    pub extends: Option<String>,
    pub implements: Vec<String>,
    pub permits: Vec<String>,
    pub members: Vec<Member>,
    pub static_init_blocks: Vec<StaticInitBlock>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceTypeNames {
    pub canonical: String,
    pub binary: String,
}

impl TypeDecl {
    /// Only a non-static member class of a class has an enclosing instance.
    /// Member types declared in interfaces, and member interfaces/enums/
    /// records themselves, are implicitly static under Java's nesting rules.
    pub(crate) fn has_enclosing_instance_in(&self, enclosing_kind: TypeKind) -> bool {
        self.kind == TypeKind::Class
            && !self.modifiers.contains(&Modifier::Static)
            && enclosing_kind != TypeKind::Interface
    }
}

/// A `static { ... }` initializer block, with the position it occupied among the
/// type's members. Java runs static field initializers and static blocks in
/// textual order (JLS 12.4.2), so class initialization needs the position, not
/// just the block.
#[derive(Debug, Clone, PartialEq)]
pub struct StaticInitBlock {
    /// Number of members already declared before this block appeared.
    pub member_index: usize,
    pub block: Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Class,
    Interface,
    Enum,
    Record,
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

impl FieldDecl {
    /// Declaration provenance, equivalent to the JVM enum-constant flag.
    /// A self-typed field or static alias is not an enum constant.
    pub fn is_enum_constant(&self) -> bool {
        self.modifiers.contains(&Modifier::EnumConstant)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MethodDecl {
    pub name: String,
    pub rust_name: Option<String>,
    pub type_params: Vec<String>,
    pub type_param_bounds: Vec<Option<JavaType>>,
    pub return_type: JavaType,
    pub throws: Vec<JavaType>,
    pub params: Vec<Param>,
    pub modifiers: Vec<Modifier>,
    pub body: Option<Block>,
    pub span: Span,
}

impl MethodDecl {
    pub fn params(&self) -> &[Param] {
        &self.params
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstructorDecl {
    pub name: String,
    pub rust_name: Option<String>,
    pub type_params: Vec<String>,
    pub type_param_bounds: Vec<Option<JavaType>>,
    pub params: Vec<Param>,
    pub throws: Vec<JavaType>,
    pub modifiers: Vec<Modifier>,
    pub body: Option<Block>,
    pub span: Span,
}

impl ConstructorDecl {
    pub fn params(&self) -> &[Param] {
        &self.params
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub source_name: String,
    pub ty: JavaType,
    pub is_varargs: bool,
    pub binding: Option<BindingId>,
}

impl Param {
    pub fn is_varargs(&self) -> bool {
        self.is_varargs
    }

    pub fn binding(&self) -> Option<&BindingId> {
        self.binding.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modifier {
    Public,
    Protected,
    Private,
    Static,
    Synchronized,
    Volatile,
    Native,
    Final,
    Sealed,
    Abstract,
    /// Set only by an enum constant declaration, not a Java source modifier.
    EnumConstant,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum JavaType {
    Void,
    Primitive(PrimitiveType),
    String,
    Array(Box<JavaType>),
    Named {
        name: String,
        args: Vec<JavaType>,
    },
    /// A Java type-parameter intersection bound such as
    /// `T extends Observation & CentroidFactory<T>`.
    ///
    /// Java permits this form only in bounds, but keeping it in the type IR
    /// prevents later analysis and Rust signature emission from silently
    /// discarding every capability after the first one.
    Intersection(Vec<JavaType>),
    Wildcard(Option<WildcardBound>),
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WildcardBound {
    Extends(Box<JavaType>),
    Super(Box<JavaType>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
        source_name: String,
        initializer: Option<Expr>,
        binding: Option<BindingId>,
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
        label: Option<String>,
    },
    DoWhile {
        body: Block,
        condition: Expr,
        label: Option<String>,
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
        source_var_name: String,
        iterable: Expr,
        body: Block,
        label: Option<String>,
        binding: Option<BindingId>,
    },
    Switch {
        selector: Expr,
        cases: Vec<SwitchCase>,
    },
    Try {
        resources: Vec<Stmt>,
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
    LabeledBlock {
        label: String,
        body: Block,
    },
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
    pub source_name: String,
    pub body: Block,
    pub binding: Option<BindingId>,
}

impl CatchClause {
    pub fn binding(&self) -> Option<&BindingId> {
        self.binding.as_ref()
    }
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
    pub constructed_type: JavaType,
    pub params: Vec<Param>,
    pub arg_coercions: Vec<JavaCoercionEvidence>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Var(String),
    This,
    Super,
    FieldAccess(Box<Expr>, String),
    ArrayAccess(Box<Expr>, Box<Expr>),
    ArrayCreation {
        element_type: JavaType,
        dimensions: Vec<Expr>,
        initializer: Option<Vec<Expr>>,
    },
    ArrayInitializer(Vec<Expr>),
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
    Switch {
        selector: Box<Expr>,
        cases: Vec<SwitchExprCase>,
    },
    Call {
        receiver: Option<Box<Expr>>,
        name: String,
        /// Explicit Java method type witnesses, kept separate from value args.
        type_args: Vec<JavaType>,
        args: Vec<Expr>,
        resolved: Option<ResolvedCall>,
    },
    New {
        ty: JavaType,
        /// True for Java diamond syntax (`new Type<>(...)`), distinct from
        /// constructing a raw generic type (`new Type(...)`).
        infer_type_arguments: bool,
        qualifier: Option<Box<Expr>>,
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
        binding: Option<String>,
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
    Unsupported {
        reason: String,
        span: Span,
    },
    Raw(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct LambdaParam {
    pub name: String,
    pub source_name: String,
    pub ty: Option<JavaType>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchExprCase {
    pub labels: Vec<SwitchLabel>,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LambdaBody {
    Expr(Box<Expr>),
    Block(Block),
}

pub(crate) fn block_mentions_this_or_super(block: &Block) -> bool {
    block.statements.iter().any(stmt_mentions_this_or_super)
}

fn stmt_mentions_this_or_super(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::VarDecl { initializer, .. } => initializer
            .as_ref()
            .is_some_and(expr_mentions_this_or_super),
        Stmt::Return(Some(expr)) => expr_mentions_this_or_super(expr),
        Stmt::Assign { target, value } => {
            expr_mentions_this_or_super(target) || expr_mentions_this_or_super(value)
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => expr_mentions_this_or_super(expr),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_mentions_this_or_super(condition)
                || block_mentions_this_or_super(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_mentions_this_or_super)
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => expr_mentions_this_or_super(condition) || block_mentions_this_or_super(body),
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(stmt_mentions_this_or_super)
                || condition.as_ref().is_some_and(expr_mentions_this_or_super)
                || update.iter().any(expr_mentions_this_or_super)
                || block_mentions_this_or_super(body)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_mentions_this_or_super(iterable) || block_mentions_this_or_super(body)
        }
        Stmt::Switch { selector, cases } => {
            expr_mentions_this_or_super(selector)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| match label {
                        SwitchLabel::Expr(expr) => expr_mentions_this_or_super(expr),
                        SwitchLabel::Default => false,
                    }) || case.statements.iter().any(stmt_mentions_this_or_super)
                })
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources.iter().any(stmt_mentions_this_or_super)
                || block_mentions_this_or_super(body)
                || catches
                    .iter()
                    .any(|catch| block_mentions_this_or_super(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_mentions_this_or_super)
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_mentions_this_or_super(block)
        }
        Stmt::ConstructorInvocation { args, .. } => args.iter().any(expr_mentions_this_or_super),
        Stmt::Return(None)
        | Stmt::LocalType(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => false,
    }
}

fn expr_mentions_this_or_super(expr: &Expr) -> bool {
    match expr {
        Expr::This | Expr::Super => true,
        Expr::FieldAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => expr_mentions_this_or_super(receiver),
        Expr::ArrayAccess(array, index) => {
            expr_mentions_this_or_super(array) || expr_mentions_this_or_super(index)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions.iter().any(expr_mentions_this_or_super)
                || initializer
                    .as_ref()
                    .is_some_and(|items| items.iter().any(expr_mentions_this_or_super))
        }
        Expr::ArrayInitializer(items) => items.iter().any(expr_mentions_this_or_super),
        Expr::Binary { left, right, .. } => {
            expr_mentions_this_or_super(left) || expr_mentions_this_or_super(right)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_mentions_this_or_super(condition)
                || expr_mentions_this_or_super(then_expr)
                || expr_mentions_this_or_super(else_expr)
        }
        Expr::Switch { selector, cases } => {
            expr_mentions_this_or_super(selector)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| match label {
                        SwitchLabel::Expr(expr) => expr_mentions_this_or_super(expr),
                        SwitchLabel::Default => false,
                    }) || expr_mentions_this_or_super(&case.value)
                })
        }
        Expr::Call { receiver, args, .. } => {
            receiver
                .as_ref()
                .is_some_and(|receiver| expr_mentions_this_or_super(receiver))
                || args.iter().any(expr_mentions_this_or_super)
        }
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            qualifier
                .as_ref()
                .is_some_and(|qualifier| expr_mentions_this_or_super(qualifier))
                || args.iter().any(expr_mentions_this_or_super)
                || body.as_ref().is_some_and(|members| {
                    members.iter().any(|member| match member {
                        Member::Method(method) => method
                            .body
                            .as_ref()
                            .is_some_and(block_mentions_this_or_super),
                        Member::Type(ty) => ty.members.iter().any(|member| match member {
                            Member::Method(method) => method
                                .body
                                .as_ref()
                                .is_some_and(block_mentions_this_or_super),
                            _ => false,
                        }),
                        _ => false,
                    })
                })
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => expr_mentions_this_or_super(expr),
            LambdaBody::Block(block) => block_mentions_this_or_super(block),
        },
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCall {
    pub declaring_type: String,
    pub rust_name: String,
    pub kind: CallKind,
    /// Explicit witnesses belonging to the selected generic method binder.
    pub type_args: Vec<JavaType>,
    pub return_type: JavaType,
    pub arg_target_types: Vec<JavaType>,
    pub arg_coercions: Vec<JavaCoercionEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallKind {
    Instance,
    Static,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaCoercionEvidence {
    pub arg_index: usize,
    pub actual: JavaType,
    pub target: JavaType,
    pub kind: JavaCoercionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JavaCoercionKind {
    Identity,
    PrimitiveWidening,
    Boxing,
    Unboxing,
    BoxingWidening,
    ReferenceWidening,
    GenericInference,
    NullToReference,
    LambdaAdaptation,
    MethodReferenceAdaptation,
    UnknownFallback,
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
    /// Java text not representable by Rust's Unicode-scalar String/char.
    /// Preserve the source value; emission must not substitute U+FFFD or zero.
    StringUtf16(Vec<u16>),
    CharUtf16(u16),
}

impl Literal {
    pub fn from_java_string_units(units: Vec<u16>) -> Self {
        match String::from_utf16(&units) {
            Ok(value) => Self::String(value),
            Err(_) => Self::StringUtf16(units),
        }
    }

    pub fn from_java_char_unit(unit: u16) -> Self {
        match char::from_u32(u32::from(unit)) {
            Some(value) => Self::Char(value),
            None => Self::CharUtf16(unit),
        }
    }

    pub fn java_char_unit(&self) -> Option<u16> {
        match self {
            Self::Char(value) => u16::try_from(u32::from(*value)).ok(),
            Self::CharUtf16(value) => Some(*value),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Assign,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    UShr,
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
    BitNot,
    Neg,
    Pos,
    PreInc,
    PreDec,
    PostInc,
    PostDec,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualified_new_receiver_mentions_this() {
        let block = Block {
            statements: vec![Stmt::Expr(Expr::New {
                infer_type_arguments: false,
                ty: JavaType::Named {
                    name: "Inner".to_string(),
                    args: Vec::new(),
                },
                qualifier: Some(Box::new(Expr::This)),
                args: Vec::new(),
                body: None,
                resolved: None,
            })],
        };

        assert!(block_mentions_this_or_super(&block));
    }
}
