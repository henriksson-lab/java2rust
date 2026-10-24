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
    pub derives: Vec<String>,
    pub fields: Vec<RustField>,
    pub is_unit: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RustField {
    pub name: String,
    pub ty: RustType,
    pub is_pub: bool,
    pub is_pub_crate: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RustImpl {
    pub target: String,
    pub trait_name: Option<String>,
    pub associated_consts: Vec<RustAssociatedConst>,
    pub functions: Vec<RustFunction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RustAssociatedConst {
    pub name: String,
    pub ty: RustType,
    pub value: RustExpr,
    pub is_pub: bool,
    pub is_pub_crate: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RustFunction {
    pub name: String,
    pub type_params: Vec<String>,
    pub params: Vec<RustParam>,
    pub return_type: Option<RustType>,
    pub where_bounds: Vec<String>,
    pub body: Vec<RustStmt>,
    pub is_pub: bool,
    pub is_pub_crate: bool,
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
    Str,
    Vec(Box<RustType>),
    Array2(Box<RustType>),
    Slice(Box<RustType>),
    Option(Box<RustType>),
    Generic {
        base: String,
        args: Vec<RustType>,
    },
    Tuple(Vec<RustType>),
    FnPointer {
        params: Vec<RustType>,
        return_type: Box<RustType>,
    },
    ImplFnTrait {
        trait_name: String,
        params: Vec<RustType>,
        return_type: Option<Box<RustType>>,
    },
    DynFnTrait {
        trait_name: String,
        params: Vec<RustType>,
        return_type: Option<Box<RustType>>,
    },
    DynTrait(String),
    ImplTrait(String),
    Named(String),
    Ref {
        mutable: bool,
        lifetime: Option<String>,
        inner: Box<RustType>,
    },
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
    AssignOp {
        target: RustExpr,
        op: String,
        value: RustExpr,
    },
    Expr(RustExpr),
    Return(Option<RustExpr>),
    If {
        condition: RustExpr,
        then_body: Vec<RustStmt>,
        else_body: Vec<RustStmt>,
    },
    IfLet {
        pattern: String,
        expr: RustExpr,
        then_body: Vec<RustStmt>,
        else_body: Vec<RustStmt>,
    },
    While {
        condition: RustExpr,
        body: Vec<RustStmt>,
    },
    WhileLet {
        label: Option<String>,
        pattern: String,
        expr: RustExpr,
        body: Vec<RustStmt>,
    },
    For {
        label: Option<String>,
        pattern: String,
        iterable: RustExpr,
        body: Vec<RustStmt>,
    },
    Loop {
        label: Option<String>,
        body: Vec<RustStmt>,
    },
    Break(Option<String>),
    BreakValue {
        label: Option<String>,
        value: RustExpr,
    },
    Continue(Option<String>),
    Raw(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RustExpr {
    Literal(String),
    Var(String),
    Path(Vec<String>),
    GenericPath {
        base: Vec<String>,
        args: Vec<RustType>,
        member: Option<String>,
    },
    TypeAssociatedPath {
        ty: RustType,
        member: String,
    },
    TraitAssociatedPath {
        ty: RustType,
        trait_name: String,
        member: String,
    },
    Field(Box<RustExpr>, String),
    Index(Box<RustExpr>, Box<RustExpr>),
    Binary {
        op: String,
        left: Box<RustExpr>,
        right: Box<RustExpr>,
    },
    Range {
        start: Box<RustExpr>,
        end: Box<RustExpr>,
        inclusive: bool,
    },
    Unary {
        op: String,
        expr: Box<RustExpr>,
    },
    Cast {
        expr: Box<RustExpr>,
        ty: String,
    },
    Try(Box<RustExpr>),
    Grouped(Box<RustExpr>),
    If {
        condition: Box<RustExpr>,
        then_expr: Box<RustExpr>,
        else_expr: Box<RustExpr>,
    },
    Tuple(Vec<RustExpr>),
    ArrayLiteral(Vec<RustExpr>),
    VecLiteral(Vec<RustExpr>),
    VecRepeat {
        value: Box<RustExpr>,
        len: Box<RustExpr>,
    },
    Call {
        function: Box<RustExpr>,
        args: Vec<RustExpr>,
    },
    MacroCall {
        macro_name: String,
        args: Vec<RustExpr>,
    },
    MethodCall {
        receiver: Box<RustExpr>,
        method: String,
        args: Vec<RustExpr>,
    },
    Closure {
        is_move: bool,
        params: Vec<RustClosureParam>,
        return_type: Option<RustType>,
        body: RustClosureBody,
    },
    Pattern(RustPattern),
    Match {
        expr: Box<RustExpr>,
        arms: Vec<RustMatchArm>,
    },
    Block {
        stmts: Vec<RustStmt>,
        tail: Option<Box<RustExpr>>,
    },
    StructLit {
        name: String,
        fields: Vec<(String, RustExpr)>,
    },
    PostfixComment {
        expr: Box<RustExpr>,
        comment: String,
    },
    UnsupportedJavaArrayCreation {
        element_type: String,
        dimensions: Vec<RustExpr>,
    },
    MarkedBlock {
        marker: String,
        expr: Box<RustExpr>,
    },
    Raw(String),
    /// An expression that remembers what the adapter which produced it made it
    /// into. Transparent everywhere else: it emits as its inner expression and
    /// has its precedence.
    ///
    /// `Raw` throws away structure, and roughly three in five constructed
    /// expressions are `Raw`, so every consumer that needs to know what an
    /// expression *is* has had one option: render it and match the text. This
    /// variant is the way out that does not require replacing `Raw` first —
    /// the payload stays a string, the answer rides alongside it.
    Shaped {
        shape: RustExprShape,
        /// The slot shape the adapter *produced*, when the producer knew it.
        ///
        /// [`RustExprShape`] is a coarse tag: it names a family
        /// (`OptionalProjected`) where several distinct adapters live
        /// (`.as_ref()`, `.as_mut()`, `.as_deref()`, `.as_deref_mut()`,
        /// `.as_ref().map(|v| v.as_str())`). That is enough to answer "what kind
        /// of thing is this" and **not** enough to answer "which of these has
        /// already been applied" — the question the idempotence checks in
        /// `apply_optional_borrowed_element_target_view` actually ask.
        ///
        /// Recording the resulting slot shape answers it exactly, which is
        /// [0007](../claude_audits/done/0007-rust-ir-is-mostly-opaque-strings.md)'s
        /// step 1: `LoweredExpr { expr, shape, actual }`. `None` means the
        /// producer did not record one — never "no shape" — so a query stays
        /// monotone.
        actual: Option<Box<crate::slot_shape::RustSlotShape>>,
        expr: Box<RustExpr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RustClosureParam {
    /// Rust pattern text. This is deliberately not restricted to identifiers:
    /// closures commonly bind tuple patterns such as `|(k, v)|`.
    pub pattern: String,
    pub ty: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RustClosureBody {
    Expr(Box<RustExpr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RustPattern {
    TupleVariantWildcard {
        enum_name: String,
        variant_name: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RustMatchArm {
    /// Rust pattern text. This keeps pattern syntax at the leaf while the
    /// scrutinee and arm bodies stay structured expressions.
    pub pattern: String,
    pub body: RustExpr,
}

/// What an adapter turned an expression into.
///
/// Each variant corresponds to a question the code currently answers by
/// matching rendered text — `rendered.ends_with(".as_str()")`,
/// `rendered.ends_with(".as_deref_mut()")`, and so on. The producer always knew
/// the answer; recording it here is what stops the consumer from guessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustExprShape {
    /// Compiler-owned null projection with an explicit Java exception payload.
    /// The catchable lowering consumes this marker, never its message text.
    ExplicitJavaNullFailure,
    /// A compiler-generated contextual NPE error value. Source rewrites can
    /// preserve the original operation without recognizing diagnostic prose
    /// or changing a source-written exception message.
    SourceNullDiagnostic,
    /// A completed Java value whose operand effects have already been lowered.
    /// It is not a Rust Result, even if evaluating its operands can throw.
    PlainValue,
    /// A slot conversion completed without producing a more specific view.
    ///
    /// This carries the resulting `RustSlotShape` so a later (for example,
    /// batch-global) consumer can recognize that ownership/dispatch/nullability
    /// adaptation has already happened.
    SlotAdapted,
    /// A Java reference whose outer Option is a nullable carrier, not a
    /// semantic java.util.Optional value. Consumers must check this carrier
    /// only at the Java operation's specified evaluation boundary.
    NullableCarrier,
    /// A `&str`: `.as_str()`, a borrowed `String`, a string literal.
    StringView,
    /// A shared borrow of the underlying value.
    Borrowed,
    /// A mutable borrow of the underlying value.
    MutBorrowed,
    /// An owned value recovered from a borrow, by clone or copy.
    OwnedRecovery,
    /// An `Option` projected to borrow or deref its element.
    OptionalProjected,
    /// Receiver of a generated Option payload map. This distinguishes its
    /// error-lifting rules from Iterator::map and Result::map.
    OptionalMapReceiver,
    /// Failure branch of a generated runtime reference-type check. The
    /// exception boundary may turn its panic fallback into ClassCastException.
    CheckedCastFailure,
    /// An `Option` constructed around a present element.
    OptionalConstructed,
    /// A shared slice view: `.as_slice()`.
    SliceView,
    /// A mutable slice view: `.as_mut_slice()`.
    MutableSliceView,
    /// An anonymous dyn-trait placeholder constructor.
    AnonymousDynTrait,
    /// A fallible expression with `?` already applied.
    ResultPropagated,
    /// A Result was discharged at an uncaught-exception boundary; this is its
    /// plain payload, even after a storage adapter wraps it.
    ResultPanicRecovered,
}

impl RustExpr {
    pub(crate) fn is_plain_value(&self) -> bool {
        match self {
            Self::Shaped {
                shape: RustExprShape::PlainValue,
                ..
            } => true,
            Self::Grouped(expr)
            | Self::Cast { expr, .. }
            | Self::Unary { expr, .. }
            | Self::PostfixComment { expr, .. }
            | Self::MarkedBlock { expr, .. } => expr.is_plain_value(),
            _ => false,
        }
    }

    /// Whether this expression propagates an error into its enclosing callable.
    /// A nested closure owns its own `?` boundary, even when immediately called.
    /// Raw leaves deliberately provide no evidence of structured effects.
    pub(crate) fn has_unbound_try(&self) -> bool {
        match self {
            Self::Try(_) => true,
            Self::Closure { .. } => false,
            Self::Field(expr, _)
            | Self::Unary { expr, .. }
            | Self::Cast { expr, .. }
            | Self::Grouped(expr)
            | Self::PostfixComment { expr, .. }
            | Self::MarkedBlock { expr, .. }
            | Self::Shaped { expr, .. } => expr.has_unbound_try(),
            Self::Index(left, right)
            | Self::Binary { left, right, .. }
            | Self::Range {
                start: left,
                end: right,
                ..
            }
            | Self::VecRepeat {
                value: left,
                len: right,
            } => left.has_unbound_try() || right.has_unbound_try(),
            Self::If {
                condition,
                then_expr,
                else_expr,
            } => {
                condition.has_unbound_try()
                    || then_expr.has_unbound_try()
                    || else_expr.has_unbound_try()
            }
            Self::Call { function, args } => {
                function.has_unbound_try() || args.iter().any(Self::has_unbound_try)
            }
            Self::MethodCall { receiver, args, .. } => {
                receiver.has_unbound_try() || args.iter().any(Self::has_unbound_try)
            }
            Self::MacroCall { args, .. }
            | Self::Tuple(args)
            | Self::ArrayLiteral(args)
            | Self::VecLiteral(args)
            | Self::UnsupportedJavaArrayCreation {
                dimensions: args, ..
            } => args.iter().any(Self::has_unbound_try),
            Self::Match { expr, arms } => {
                expr.has_unbound_try() || arms.iter().any(|arm| arm.body.has_unbound_try())
            }
            Self::Block { stmts, tail } => {
                stmts.iter().any(RustStmt::has_unbound_try)
                    || tail.as_deref().is_some_and(Self::has_unbound_try)
            }
            Self::StructLit { fields, .. } => {
                fields.iter().any(|(_, value)| value.has_unbound_try())
            }
            Self::Literal(_)
            | Self::Var(_)
            | Self::Path(_)
            | Self::GenericPath { .. }
            | Self::TypeAssociatedPath { .. }
            | Self::TraitAssociatedPath { .. }
            | Self::Pattern(_)
            | Self::Raw(_) => false,
        }
    }

    pub(crate) fn raw_leaf(rendered: impl Into<String>) -> RustExpr {
        RustExpr::Raw(rendered.into())
    }

    pub fn path<I, S>(segments: I) -> RustExpr
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        RustExpr::Path(segments.into_iter().map(Into::into).collect())
    }

    pub fn generic_path<I, S, M>(base: I, args: Vec<RustType>, member: Option<M>) -> RustExpr
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
        M: Into<String>,
    {
        RustExpr::GenericPath {
            base: base.into_iter().map(Into::into).collect(),
            args,
            member: member.map(Into::into),
        }
    }

    /// Tag an expression with what an adapter made it. Re-tagging replaces the
    /// previous tag rather than nesting: the shape of an expression is what the
    /// *last* adapter made it.
    pub fn shaped(shape: RustExprShape, expr: RustExpr) -> RustExpr {
        RustExpr::shaped_as(shape, None, expr)
    }

    /// Tag an expression with its family *and* the slot shape it produced.
    ///
    /// The `actual` is what lets a consumer ask "is this already the shape I
    /// want" instead of "is this in the family of things that might be".
    pub fn shaped_as(
        shape: RustExprShape,
        actual: Option<crate::slot_shape::RustSlotShape>,
        expr: RustExpr,
    ) -> RustExpr {
        RustExpr::Shaped {
            shape,
            actual: actual.map(Box::new),
            expr: Box::new(match expr {
                RustExpr::Shaped { expr, .. } => *expr,
                other => other,
            }),
        }
    }

    /// The slot shape the last adapter produced, if it recorded one.
    pub fn recorded_slot_shape(&self) -> Option<&crate::slot_shape::RustSlotShape> {
        match self {
            RustExpr::Shaped { actual, .. } => actual.as_deref(),
            _ => None,
        }
    }

    /// The shape recorded when this expression was built, if any.
    ///
    /// `None` means "not recorded", never "not that shape" — most expressions
    /// are still untagged, so a consumer must treat `None` as unknown and fall
    /// back rather than as a negative answer.
    pub fn recorded_shape(&self) -> Option<RustExprShape> {
        match self {
            RustExpr::Shaped { shape, .. } => Some(*shape),
            _ => None,
        }
    }

    /// The expression underneath any shape tag.
    ///
    /// Consumers that match on the structure of an expression call this first,
    /// so tagging a producer cannot change what they see.
    pub fn unshaped(&self) -> &RustExpr {
        match self {
            RustExpr::Shaped { expr, .. } => expr.unshaped(),
            other => other,
        }
    }

    /// Whether this expression is still an opaque raw leaf after removing
    /// compatibility shape tags.
    pub(crate) fn is_raw_leaf(&self) -> bool {
        matches!(self.unshaped(), RustExpr::Raw(_))
    }

    /// [`Self::unshaped`] by value.
    pub fn into_unshaped(self) -> RustExpr {
        match self {
            RustExpr::Shaped { expr, .. } => expr.into_unshaped(),
            other => other,
        }
    }
}

pub(crate) fn rust_type_arg_projection_or_raw(rust_ty: &str) -> RustType {
    match rust_ty.trim() {
        "()" => RustType::Unit,
        "bool" => RustType::Bool,
        "i8" => RustType::I8,
        "i16" => RustType::I16,
        "i32" => RustType::I32,
        "i64" => RustType::I64,
        "f32" => RustType::F32,
        "f64" => RustType::F64,
        "char" => RustType::Char,
        "String" => RustType::String,
        "str" => RustType::Str,
        trimmed if trimmed.starts_with('&') => rust_ref_type_projection_or_raw(trimmed),
        trimmed if let Some(items) = rust_tuple_type_projection(trimmed) => RustType::Tuple(items),
        trimmed if let Some(fn_pointer) = rust_fn_pointer_type_projection(trimmed) => fn_pointer,
        trimmed if let Some(trait_name) = trimmed.strip_prefix("dyn ") => {
            rust_trait_type_projection_or_raw("dyn", trait_name, RustType::DynTrait)
        }
        trimmed if let Some(trait_name) = trimmed.strip_prefix("impl ") => {
            rust_trait_type_projection_or_raw("impl", trait_name, RustType::ImplTrait)
        }
        trimmed if let Some((base, args)) = split_simple_rust_generic_type(trimmed) => {
            let args = args
                .into_iter()
                .map(rust_type_arg_projection_or_raw)
                .collect::<Vec<_>>();
            if args.iter().any(|arg| matches!(arg, RustType::Raw(_))) {
                return RustType::Raw(trimmed.to_string());
            }
            match (base, args.as_slice()) {
                ("Vec", [inner]) => RustType::Vec(Box::new(inner.clone())),
                ("Option", [inner]) => RustType::Option(Box::new(inner.clone())),
                ("ndarray::Array2", [inner]) => RustType::Array2(Box::new(inner.clone())),
                _ => RustType::Generic {
                    base: base.to_string(),
                    args,
                },
            }
        }
        trimmed if rust_type_ident_is_plain(trimmed) => RustType::Named(trimmed.to_string()),
        trimmed => RustType::Raw(trimmed.to_string()),
    }
}

fn rust_trait_type_projection_or_raw(
    prefix: &str,
    trait_name: &str,
    projection: impl FnOnce(String) -> RustType,
) -> RustType {
    let trait_name = trait_name.trim();
    if rust_type_path_is_simple(trait_name) {
        projection(trait_name.to_string())
    } else {
        RustType::Raw(format!("{prefix} {trait_name}"))
    }
}

fn rust_ref_type_projection_or_raw(rendered: &str) -> RustType {
    let Some(rest) = rendered.strip_prefix('&') else {
        return RustType::Raw(rendered.to_string());
    };
    let mut rest = rest.trim_start();
    let lifetime = if rest.starts_with('\'') {
        let Some((candidate, tail)) = split_once_whitespace(rest) else {
            return RustType::Raw(rendered.to_string());
        };
        if !rust_lifetime_is_simple(candidate) {
            return RustType::Raw(rendered.to_string());
        }
        rest = tail.trim_start();
        Some(candidate.to_string())
    } else {
        None
    };
    let mutable = if let Some(tail) = rest.strip_prefix("mut ") {
        rest = tail.trim_start();
        true
    } else {
        false
    };
    if rest.is_empty() {
        return RustType::Raw(rendered.to_string());
    }
    let inner = rust_type_arg_projection_or_raw(rest);
    if matches!(inner, RustType::Raw(_)) {
        return RustType::Raw(rendered.to_string());
    }
    RustType::Ref {
        mutable,
        lifetime,
        inner: Box::new(inner),
    }
}

fn rust_fn_pointer_type_projection(rendered: &str) -> Option<RustType> {
    let rest = rendered.strip_prefix("fn(")?;
    let close = matching_top_level_paren(rest)?;
    let params = &rest[..close];
    let tail = rest[close + 1..].trim();
    let return_type = if tail.is_empty() {
        RustType::Unit
    } else {
        let return_text = tail.strip_prefix("->")?.trim();
        if return_text.is_empty() {
            return None;
        }
        rust_type_arg_projection_or_raw(return_text)
    };
    if matches!(return_type, RustType::Raw(_)) {
        return None;
    }
    let params = if params.trim().is_empty() {
        Vec::new()
    } else {
        let projected = split_top_level_type_args(params)?
            .into_iter()
            .map(rust_type_arg_projection_or_raw)
            .collect::<Vec<_>>();
        if projected
            .iter()
            .any(|param| matches!(param, RustType::Raw(_)))
        {
            return None;
        }
        projected
    };
    Some(RustType::FnPointer {
        params,
        return_type: Box::new(return_type),
    })
}

fn matching_top_level_paren(rest_after_open: &str) -> Option<usize> {
    let mut depth = 1usize;
    for (index, ch) in rest_after_open.char_indices() {
        match ch {
            '(' => depth = depth.checked_add(1)?,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_once_whitespace(value: &str) -> Option<(&str, &str)> {
    let split = value.find(char::is_whitespace)?;
    Some((&value[..split], &value[split..]))
}

fn rust_lifetime_is_simple(lifetime: &str) -> bool {
    let Some(name) = lifetime.strip_prefix('\'') else {
        return false;
    };
    !name.is_empty() && rust_type_ident_is_plain(name)
}

fn rust_type_ident_is_plain(segment: &str) -> bool {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn rust_type_path_is_simple(path: &str) -> bool {
    path.split("::").all(rust_type_ident_is_plain)
}

fn split_simple_rust_generic_type(rendered: &str) -> Option<(&str, Vec<&str>)> {
    let open = rendered.find('<')?;
    if !rendered.ends_with('>') {
        return None;
    }
    let base = rendered[..open].trim();
    if !rust_type_path_is_simple(base) {
        return None;
    }
    let inner = &rendered[open + 1..rendered.len() - 1];
    let args = split_top_level_type_args(inner)?;
    (!args.is_empty()).then_some((base, args))
}

fn split_top_level_type_args(rendered: &str) -> Option<Vec<&str>> {
    let mut args = Vec::new();
    let mut angle_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut start = 0usize;
    for (index, ch) in rendered.char_indices() {
        match ch {
            '<' => angle_depth = angle_depth.checked_add(1)?,
            '>' if rendered[..index].ends_with('-') => {}
            '>' => {
                angle_depth = angle_depth.checked_sub(1)?;
            }
            '(' => paren_depth = paren_depth.checked_add(1)?,
            ')' => {
                paren_depth = paren_depth.checked_sub(1)?;
            }
            '[' => bracket_depth = bracket_depth.checked_add(1)?,
            ']' => {
                bracket_depth = bracket_depth.checked_sub(1)?;
            }
            ',' if angle_depth == 0 && paren_depth == 0 && bracket_depth == 0 => {
                let arg = rendered[start..index].trim();
                if arg.is_empty() {
                    return None;
                }
                args.push(arg);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    if angle_depth != 0 || paren_depth != 0 || bracket_depth != 0 {
        return None;
    }
    let arg = rendered[start..].trim();
    if arg.is_empty() {
        return None;
    }
    args.push(arg);
    Some(args)
}

fn rust_tuple_type_projection(rendered: &str) -> Option<Vec<RustType>> {
    if !rendered.starts_with('(') || !rendered.ends_with(')') {
        return None;
    }
    let inner = &rendered[1..rendered.len() - 1];
    let items = split_top_level_type_args(inner)?;
    if items.len() < 2 {
        return None;
    }
    let projected = items
        .into_iter()
        .map(rust_type_arg_projection_or_raw)
        .collect::<Vec<_>>();
    projected
        .iter()
        .all(|item| !matches!(item, RustType::Raw(_)))
        .then_some(projected)
}

impl RustStmt {
    fn has_unbound_try(&self) -> bool {
        match self {
            Self::Let { value, .. } | Self::Return(value) => {
                value.as_ref().is_some_and(RustExpr::has_unbound_try)
            }
            Self::Assign { target, value } | Self::AssignOp { target, value, .. } => {
                target.has_unbound_try() || value.has_unbound_try()
            }
            Self::Expr(expr) | Self::BreakValue { value: expr, .. } => expr.has_unbound_try(),
            Self::If {
                condition,
                then_body,
                else_body,
            } => {
                condition.has_unbound_try()
                    || then_body.iter().any(Self::has_unbound_try)
                    || else_body.iter().any(Self::has_unbound_try)
            }
            Self::IfLet {
                expr,
                then_body,
                else_body,
                ..
            } => {
                expr.has_unbound_try()
                    || then_body.iter().any(Self::has_unbound_try)
                    || else_body.iter().any(Self::has_unbound_try)
            }
            Self::While { condition, body } => {
                condition.has_unbound_try() || body.iter().any(Self::has_unbound_try)
            }
            Self::WhileLet { expr, body, .. }
            | Self::For {
                iterable: expr,
                body,
                ..
            } => expr.has_unbound_try() || body.iter().any(Self::has_unbound_try),
            Self::Loop { body, .. } => body.iter().any(Self::has_unbound_try),
            Self::Break(_) | Self::Continue(_) | Self::Raw(_) => false,
        }
    }

    pub(crate) fn raw_leaf(rendered: impl Into<String>) -> RustStmt {
        RustStmt::Raw(rendered.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit::emit_expr;

    #[test]
    fn stream_callback_unbound_try_effect_respects_callable_and_statement_scopes() {
        let fallible = RustExpr::Try(Box::new(RustExpr::path(["value"])));
        let closure = RustExpr::Closure {
            is_move: false,
            params: Vec::new(),
            return_type: None,
            body: RustClosureBody::Expr(Box::new(fallible.clone())),
        };
        assert!(!closure.has_unbound_try());
        let call = RustExpr::Call {
            function: Box::new(closure),
            args: Vec::new(),
        };
        assert!(!call.has_unbound_try());
        assert!(RustExpr::Try(Box::new(call.clone())).has_unbound_try());
        assert!(
            RustExpr::Call {
                function: Box::new(call),
                args: vec![fallible.clone()]
            }
            .has_unbound_try()
        );
        assert!(
            RustExpr::Block {
                stmts: vec![RustStmt::For {
                    label: None,
                    pattern: "item".to_string(),
                    iterable: RustExpr::path(["items"]),
                    body: vec![RustStmt::Let {
                        mutable: false,
                        name: "value".to_string(),
                        ty: None,
                        value: Some(fallible)
                    }],
                }],
                tail: None,
            }
            .has_unbound_try()
        );
        assert!(!RustExpr::Literal("\"question?\"".to_string()).has_unbound_try());
        assert!(!RustExpr::raw_leaf("opaque?").has_unbound_try());
    }

    /// A tag must not reach the output. If it did, every tagged producer would
    /// change the generated Rust and the migration could not be incremental.
    #[test]
    fn a_shape_tag_is_invisible_to_the_emitter() {
        let plain = RustExpr::Raw("name.as_str()".to_string());
        let tagged = RustExpr::shaped(RustExprShape::StringView, plain.clone());
        assert_eq!(emit_expr(&tagged), emit_expr(&plain));
    }

    /// Tagging is idempotent and last-writer-wins: an expression has one shape,
    /// whatever the most recent adapter made it, not a stack of them.
    #[test]
    fn re_tagging_replaces_rather_than_nests() {
        let once = RustExpr::shaped(RustExprShape::Borrowed, RustExpr::Raw("value".to_string()));
        let twice = RustExpr::shaped(RustExprShape::StringView, once);
        assert_eq!(twice.recorded_shape(), Some(RustExprShape::StringView));
        assert_eq!(twice.unshaped(), &RustExpr::Raw("value".to_string()));
    }

    /// Structural consumers see through the tag, so adding one to a producer
    /// cannot silently change a match somewhere downstream.
    #[test]
    fn structural_consumers_see_through_the_tag() {
        let var = RustExpr::Var("x".to_string());
        let tagged = RustExpr::shaped(RustExprShape::Borrowed, var.clone());
        assert_eq!(tagged.unshaped(), &var);
        assert_eq!(tagged.into_unshaped(), var);
    }

    #[test]
    fn raw_type_arg_projection_recovers_simple_generic_and_trait_types() {
        assert_eq!(
            rust_type_arg_projection_or_raw("Box<dyn std::any::Any>"),
            RustType::Generic {
                base: "Box".to_string(),
                args: vec![RustType::DynTrait("std::any::Any".to_string())],
            }
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("Vec<Option<String>>"),
            RustType::Vec(Box::new(RustType::Option(Box::new(RustType::String))))
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("ndarray::Array2<i32>"),
            RustType::Array2(Box::new(RustType::I32))
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("impl java::Thing"),
            RustType::ImplTrait("java::Thing".to_string())
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("&'static str"),
            RustType::Ref {
                mutable: false,
                lifetime: Some("'static".to_string()),
                inner: Box::new(RustType::Str),
            }
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("&mut Vec<String>"),
            RustType::Ref {
                mutable: true,
                lifetime: None,
                inner: Box::new(RustType::Vec(Box::new(RustType::String))),
            }
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("(String, i32)"),
            RustType::Tuple(vec![RustType::String, RustType::I32])
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("Vec<(String, i32)>"),
            RustType::Vec(Box::new(RustType::Tuple(vec![
                RustType::String,
                RustType::I32
            ]))),
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("fn(i32, String)"),
            RustType::FnPointer {
                params: vec![RustType::I32, RustType::String],
                return_type: Box::new(RustType::Unit),
            }
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("fn(&str, Vec<i32>) -> Option<String>"),
            RustType::FnPointer {
                params: vec![
                    RustType::Ref {
                        mutable: false,
                        lifetime: None,
                        inner: Box::new(RustType::Str),
                    },
                    RustType::Vec(Box::new(RustType::I32)),
                ],
                return_type: Box::new(RustType::Option(Box::new(RustType::String))),
            }
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("Option<fn(i32) -> bool>"),
            RustType::Option(Box::new(RustType::FnPointer {
                params: vec![RustType::I32],
                return_type: Box::new(RustType::Bool),
            }))
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("std::rc::Rc<std::cell::RefCell<String>>"),
            RustType::Generic {
                base: "std::rc::Rc".to_string(),
                args: vec![RustType::Generic {
                    base: "std::cell::RefCell".to_string(),
                    args: vec![RustType::String],
                }],
            }
        );
    }

    #[test]
    fn raw_type_arg_projection_keeps_malformed_generics_raw() {
        assert_eq!(
            rust_type_arg_projection_or_raw("Vec<String"),
            RustType::Raw("Vec<String".to_string())
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("Vec<>"),
            RustType::Raw("Vec<>".to_string())
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("Vec<dyn Fn(i32)>"),
            RustType::Raw("Vec<dyn Fn(i32)>".to_string())
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("Option<dyn Fn(i32)>"),
            RustType::Raw("Option<dyn Fn(i32)>".to_string())
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("dyn Fn(i32)"),
            RustType::Raw("dyn Fn(i32)".to_string())
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("fn(i32"),
            RustType::Raw("fn(i32".to_string())
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("fn(i32) -> dyn Fn(i32)"),
            RustType::Raw("fn(i32) -> dyn Fn(i32)".to_string())
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("&"),
            RustType::Raw("&".to_string())
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("&'bad-lifetime str"),
            RustType::Raw("&'bad-lifetime str".to_string())
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("&Vec<String"),
            RustType::Raw("&Vec<String".to_string())
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("(String)"),
            RustType::Raw("(String)".to_string())
        );
        assert_eq!(
            rust_type_arg_projection_or_raw("(String, Vec<i32>"),
            RustType::Raw("(String, Vec<i32>".to_string())
        );
    }
}
