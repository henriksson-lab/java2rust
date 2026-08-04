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
    pub type_params: Vec<String>,
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
    Array2(Box<RustType>),
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
    Cast {
        expr: Box<RustExpr>,
        ty: String,
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
        expr: Box<RustExpr>,
    },
}

/// What an adapter turned an expression into.
///
/// Each variant corresponds to a question the code currently answers by
/// matching rendered text — `rendered.ends_with(".as_str()")`,
/// `rendered.ends_with(".as_deref_mut()")`, and so on. The producer always knew
/// the answer; recording it here is what stops the consumer from guessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustExprShape {
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
    /// A slice view: `.as_slice()`, `.as_mut_slice()`.
    SliceView,
    /// A fallible expression with `?` already applied.
    ResultPropagated,
}

impl RustExpr {
    /// Tag an expression with what an adapter made it. Re-tagging replaces the
    /// previous tag rather than nesting: the shape of an expression is what the
    /// *last* adapter made it.
    pub fn shaped(shape: RustExprShape, expr: RustExpr) -> RustExpr {
        RustExpr::Shaped {
            shape,
            expr: Box::new(match expr {
                RustExpr::Shaped { expr, .. } => *expr,
                other => other,
            }),
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

    /// [`Self::unshaped`] by value.
    pub fn into_unshaped(self) -> RustExpr {
        match self {
            RustExpr::Shaped { expr, .. } => expr.into_unshaped(),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit::emit_expr;

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
}
