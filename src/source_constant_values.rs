//! Values of `static final` fields declared in *available* source.
//!
//! A constant the translator has no value for used to become `0`, `false` or
//! `""` — placeholders that are indistinguishable from real values, pass type
//! checking, and change behaviour rather than stopping it. That is now a loud
//! `compile_error!` naming the owner and field, which is the right *default*.
//!
//! But it is the wrong answer when the value is sitting in source the
//! translation is already reading. A class from the corpus's own dependency
//! set, supplied as a context-summary source, contributes its field *types* to
//! the fact set and not its *initialisers*, so `SIZEOF_BCF_HEADER = 9` reached
//! the facade with no value at all — group 3 of
//! [0031](../claude_audits/done/0031-stubbed-constant-census.md). The information was
//! present and unused, which is the same shape as
//! [0033](../claude_audits/done/0033-receiver-mutability-not-propagated-through-lambdas.md).
//!
//! This module folds a Java *constant expression* (JLS 15.28) into the Rust
//! literal that facade emission wants. It is deliberately conservative: an
//! expression it cannot prove constant yields `None` and the loud diagnostic
//! stands. Being wrong here would be worse than being silent, because a wrong
//! constant is exactly the failure this whole audit is about.

use std::collections::BTreeMap;

use crate::java_ir::{BinaryOp, Expr, JavaType, Literal, PrimitiveType, UnaryOp};

/// A folded Java constant, in Java's own value space.
///
/// Kept separate from the Rust rendering so that arithmetic happens with Java's
/// widths and overflow behaviour rather than the target type's.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ConstantValue {
    Bool(bool),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    Char(u16),
    String(Vec<u16>),
}

impl ConstantValue {
    /// The Rust expression text for this value at `java_ty`.
    ///
    /// Returns `None` when the declared type and the folded value disagree —
    /// a mismatch means the fold was wrong, and emitting anything would defeat
    /// the point of the module.
    pub(crate) fn rust_value_for(&self, java_ty: &JavaType) -> Option<String> {
        match (java_ty, self) {
            (JavaType::Primitive(PrimitiveType::Boolean), Self::Bool(value)) => {
                Some(value.to_string())
            }
            (JavaType::Primitive(PrimitiveType::Byte), Self::Int(value)) => {
                i8::try_from(*value).ok().map(|value| value.to_string())
            }
            (JavaType::Primitive(PrimitiveType::Short), Self::Int(value)) => {
                i16::try_from(*value).ok().map(|value| value.to_string())
            }
            (JavaType::Primitive(PrimitiveType::Int), Self::Int(value)) => Some(value.to_string()),
            (JavaType::Primitive(PrimitiveType::Long), Self::Long(value)) => {
                Some(format!("{value}i64"))
            }
            // Java widens an `int` constant to `long` without a cast.
            (JavaType::Primitive(PrimitiveType::Long), Self::Int(value)) => {
                Some(format!("{value}i64"))
            }
            (JavaType::Primitive(PrimitiveType::Float), value) => {
                float_value(value).map(|value| render_f32(value))
            }
            (JavaType::Primitive(PrimitiveType::Double), value) => {
                double_value(value).map(|value| render_f64(value))
            }
            (JavaType::Primitive(PrimitiveType::Char), Self::Char(value)) => {
                char::from_u32(u32::from(*value)).map(|value| format!("{value:?}"))
            }
            (JavaType::String, Self::String(value)) => String::from_utf16(value)
                .ok()
                .map(|value| format!("{value:?}")),
            _ => None,
        }
    }
}

fn float_value(value: &ConstantValue) -> Option<f32> {
    match value {
        ConstantValue::Float(value) => Some(*value),
        ConstantValue::Double(value) => Some(*value as f32),
        ConstantValue::Int(value) => Some(*value as f32),
        ConstantValue::Long(value) => Some(*value as f32),
        ConstantValue::Char(value) => Some(f32::from(*value)),
        _ => None,
    }
}

fn double_value(value: &ConstantValue) -> Option<f64> {
    match value {
        ConstantValue::Double(value) => Some(*value),
        ConstantValue::Float(value) => Some(*value as f64),
        ConstantValue::Int(value) => Some(*value as f64),
        ConstantValue::Long(value) => Some(*value as f64),
        ConstantValue::Char(value) => Some(f64::from(*value)),
        _ => None,
    }
}

/// `1.0` must not render as `1`, which is not an `f64` literal in Rust, and
/// infinities and NaN have no literal form at all.
fn render_f64(value: f64) -> String {
    if value.is_nan() {
        return "f64::NAN".to_string();
    }
    if value.is_infinite() {
        return if value.is_sign_positive() {
            "f64::INFINITY".to_string()
        } else {
            "f64::NEG_INFINITY".to_string()
        };
    }
    let text = format!("{value:?}");
    if text.contains('.') || text.contains('e') || text.contains('E') {
        text
    } else {
        format!("{text}.0")
    }
}

fn render_f32(value: f32) -> String {
    if value.is_nan() {
        return "f32::NAN".to_string();
    }
    if value.is_infinite() {
        return if value.is_sign_positive() {
            "f32::INFINITY".to_string()
        } else {
            "f32::NEG_INFINITY".to_string()
        };
    }
    format!("{value:?}f32")
}

/// Constants already folded, indexed so a reference resolves the way Java's
/// scoping rules say it should.
///
/// Recording simple owner names in one flat map was **wrong**, and silently so:
/// `pkg1.Constants.X` and `pkg2.Constants.X` both wrote the key `Constants.X`,
/// last one winning, so a class in `pkg1` referring to `Constants.X` could fold
/// to `pkg2`'s value. A constant that is confidently the wrong number is
/// precisely what
/// [0031](../claude_audits/done/0031-stubbed-constant-census.md) exists to
/// prevent, so the resolution order is now explicit:
///
/// 1. a fully-qualified path always wins;
/// 2. a simple owner name resolves **within the referring type's package**, as
///    Java does;
/// 3. failing that it must be unambiguous across everything recorded;
/// 4. otherwise nothing folds and the loud `compile_error!` stands.
#[derive(Debug, Clone, Default)]
pub(crate) struct ConstantScope {
    /// Fully-qualified `pkg.Owner.FIELD`, in every alias spelling.
    qualified: BTreeMap<String, ConstantValue>,
    /// `(simple owner, field alias)` -> the qualified owners that declare it.
    /// A map rather than a value so ambiguity is *detectable* instead of being
    /// resolved by whichever file was parsed last.
    by_simple: BTreeMap<(String, String), BTreeMap<String, ConstantValue>>,
    /// Bare names visible to the owner currently being folded.
    own: BTreeMap<String, ConstantValue>,
    /// That owner's package, which decides which same-simple-name owner a
    /// simple reference means.
    package: Option<String>,
}

impl ConstantScope {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Records `value` under every spelling a reference to it can take.
    ///
    /// The parser snake-cases identifiers, so `SIZEOF_BCF_HEADER` arrives as
    /// `Var("sizeof_bcf_header")` and `CommonInfo.X` as
    /// `FieldAccess(Var("common_info"), "X")`. Rather than guess which spelling
    /// a given reference will use, every one is recorded.
    pub(crate) fn record(&mut self, owner: &str, field_name: &str, value: ConstantValue) {
        let simple_owner = owner.rsplit('.').next().unwrap_or(owner);
        for field_alias in name_aliases(field_name) {
            for owner_alias in name_aliases(owner) {
                self.qualified
                    .insert(format!("{owner_alias}.{field_alias}"), value.clone());
            }
            for simple_alias in name_aliases(simple_owner) {
                self.by_simple
                    .entry((simple_alias, field_alias.clone()))
                    .or_default()
                    .insert(owner.to_string(), value.clone());
            }
        }
    }

    /// Points the scope at the owner whose declarations are being folded.
    ///
    /// `own` are that owner's already-folded constants, which Java makes
    /// visible as bare names inside it.
    pub(crate) fn focus_on_owner(
        &mut self,
        package: Option<&str>,
        own: BTreeMap<String, ConstantValue>,
    ) {
        self.package = package.map(str::to_string);
        self.own = own;
    }

    fn lookup_bare(&self, name: &str) -> Option<ConstantValue> {
        self.own.get(name).cloned()
    }

    fn lookup_path(&self, path: &str) -> Option<ConstantValue> {
        if let Some(value) = self.qualified.get(path) {
            return Some(value.clone());
        }
        let (owner, field) = path.rsplit_once('.')?;
        // Only a bare `Owner.FIELD` can be package-relative; a longer path that
        // did not match `qualified` above names something not recorded.
        if owner.contains('.') {
            return None;
        }
        let candidates = self
            .by_simple
            .get(&(owner.to_string(), field.to_string()))?;
        if let Some(package) = &self.package
            && let Some(value) = candidates.get(&format!("{package}.{owner}"))
        {
            return Some(value.clone());
        }
        if candidates.len() == 1 {
            return candidates.values().next().cloned();
        }
        // Ambiguous, and nothing in scope says which one is meant. Refusing is
        // the whole point: the alternative is a plausible wrong number.
        None
    }
}

fn name_aliases(name: &str) -> Vec<String> {
    let snake = crate::parser::to_snake_case(name);
    if snake == name {
        vec![name.to_string()]
    } else {
        vec![name.to_string(), snake]
    }
}

/// The dotted text of a reference, or `None` if it is not a plain dotted path.
fn dotted_path(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Var(name) => Some(name.clone()),
        Expr::FieldAccess(receiver, field) => Some(format!("{}.{field}", dotted_path(receiver)?)),
        _ => None,
    }
}

/// Folds `expr` if — and only if — it is a Java constant expression.
pub(crate) fn fold_constant_expr(expr: &Expr, scope: &ConstantScope) -> Option<ConstantValue> {
    match expr {
        Expr::Literal(literal) => fold_literal(literal),
        // A bare name is a constant only if it is one of the constants already
        // folded for this owner. Anything else (a field of another class, a
        // parameter, a static import) is out of scope by construction.
        Expr::Var(name) => scope.lookup_bare(name),
        // A qualified reference to another owner's constant. Java requires the
        // referent to be a constant too, so this stays inside JLS 15.28.
        Expr::FieldAccess(..) => scope.lookup_path(&dotted_path(expr)?),
        Expr::Unary { op, expr } => fold_unary(*op, fold_constant_expr(expr, scope)?),
        Expr::Binary { op, left, right } => fold_binary(
            *op,
            fold_constant_expr(left, scope)?,
            fold_constant_expr(right, scope)?,
        ),
        Expr::Cast { ty, expr } => fold_cast(ty, fold_constant_expr(expr, scope)?),
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => match fold_constant_expr(condition, scope)? {
            ConstantValue::Bool(true) => fold_constant_expr(then_expr, scope),
            ConstantValue::Bool(false) => fold_constant_expr(else_expr, scope),
            _ => None,
        },
        _ => None,
    }
}

fn fold_literal(literal: &Literal) -> Option<ConstantValue> {
    match literal {
        Literal::Bool(value) => Some(ConstantValue::Bool(*value)),
        Literal::Int(value) => Some(ConstantValue::Int(*value)),
        Literal::Long(value) => Some(ConstantValue::Long(*value)),
        Literal::Float(value) => Some(ConstantValue::Float(*value)),
        Literal::Double(value) => Some(ConstantValue::Double(*value)),
        Literal::Char(value) => Some(ConstantValue::Char(u16::try_from(u32::from(*value)).ok()?)),
        Literal::CharUtf16(value) => Some(ConstantValue::Char(*value)),
        Literal::String(value) => Some(ConstantValue::String(value.encode_utf16().collect())),
        Literal::StringUtf16(value) => Some(ConstantValue::String(value.clone())),
        // `null` is not a constant expression in the JLS sense, and a `null`
        // constant is precisely a case that must stay loud.
        Literal::Null => None,
    }
}

fn fold_unary(op: UnaryOp, value: ConstantValue) -> Option<ConstantValue> {
    // Unary numeric promotion widens a Java char to int, including surrogates.
    let value = match value {
        ConstantValue::Char(value) => ConstantValue::Int(i32::from(value)),
        other => other,
    };
    match (op, value) {
        (UnaryOp::Pos, value) => Some(value),
        (UnaryOp::Neg, ConstantValue::Int(value)) => Some(ConstantValue::Int(value.wrapping_neg())),
        (UnaryOp::Neg, ConstantValue::Long(value)) => {
            Some(ConstantValue::Long(value.wrapping_neg()))
        }
        (UnaryOp::Neg, ConstantValue::Float(value)) => Some(ConstantValue::Float(-value)),
        (UnaryOp::Neg, ConstantValue::Double(value)) => Some(ConstantValue::Double(-value)),
        (UnaryOp::Not, ConstantValue::Bool(value)) => Some(ConstantValue::Bool(!value)),
        (UnaryOp::BitNot, ConstantValue::Int(value)) => Some(ConstantValue::Int(!value)),
        (UnaryOp::BitNot, ConstantValue::Long(value)) => Some(ConstantValue::Long(!value)),
        // `++`/`--` are not constant expressions.
        _ => None,
    }
}

fn fold_cast(ty: &JavaType, value: ConstantValue) -> Option<ConstantValue> {
    let JavaType::Primitive(primitive) = ty else {
        // A reference cast of a constant is not a constant expression, except
        // `(String)` — which is a no-op the fold does not need.
        return None;
    };
    let as_long = match &value {
        ConstantValue::Int(value) => Some(i64::from(*value)),
        ConstantValue::Long(value) => Some(*value),
        ConstantValue::Char(value) => Some(i64::from(*value)),
        _ => None,
    };
    match primitive {
        PrimitiveType::Byte => Some(ConstantValue::Int(i32::from(as_long? as i8))),
        PrimitiveType::Short => Some(ConstantValue::Int(i32::from(as_long? as i16))),
        PrimitiveType::Int => match &value {
            ConstantValue::Float(value) => Some(ConstantValue::Int(*value as i32)),
            ConstantValue::Double(value) => Some(ConstantValue::Int(*value as i32)),
            _ => Some(ConstantValue::Int(as_long? as i32)),
        },
        PrimitiveType::Long => match &value {
            ConstantValue::Float(value) => Some(ConstantValue::Long(*value as i64)),
            ConstantValue::Double(value) => Some(ConstantValue::Long(*value as i64)),
            _ => Some(ConstantValue::Long(as_long?)),
        },
        PrimitiveType::Float => Some(ConstantValue::Float(float_value(&value)?)),
        PrimitiveType::Double => Some(ConstantValue::Double(double_value(&value)?)),
        PrimitiveType::Char => Some(ConstantValue::Char(match value {
            // Java first narrows floating-point to int (saturating/NaN->0),
            // then keeps its low 16 bits; integral narrowing wraps directly.
            ConstantValue::Float(value) => value as i32 as u16,
            ConstantValue::Double(value) => value as i32 as u16,
            _ => as_long? as u16,
        })),
        PrimitiveType::Boolean => match value {
            ConstantValue::Bool(value) => Some(ConstantValue::Bool(value)),
            _ => None,
        },
    }
}

fn fold_binary(op: BinaryOp, left: ConstantValue, right: ConstantValue) -> Option<ConstantValue> {
    // String concatenation of constants is itself a constant expression.
    if op == BinaryOp::Add
        && (matches!(left, ConstantValue::String(_)) || matches!(right, ConstantValue::String(_)))
    {
        let mut units = java_display(&left)?;
        units.extend(java_display(&right)?);
        return Some(ConstantValue::String(units));
    }
    match (&left, &right) {
        (ConstantValue::Bool(left), ConstantValue::Bool(right)) => match op {
            BinaryOp::And => Some(ConstantValue::Bool(*left && *right)),
            BinaryOp::Or => Some(ConstantValue::Bool(*left || *right)),
            BinaryOp::BitAnd => Some(ConstantValue::Bool(*left & *right)),
            BinaryOp::BitOr => Some(ConstantValue::Bool(*left | *right)),
            BinaryOp::BitXor => Some(ConstantValue::Bool(*left ^ *right)),
            BinaryOp::Eq => Some(ConstantValue::Bool(left == right)),
            BinaryOp::Ne => Some(ConstantValue::Bool(left != right)),
            _ => None,
        },
        _ => fold_numeric_binary(op, &left, &right),
    }
}

/// Java's binary numeric promotion, restricted to the constant case.
fn fold_numeric_binary(
    op: BinaryOp,
    left: &ConstantValue,
    right: &ConstantValue,
) -> Option<ConstantValue> {
    if matches!(left, ConstantValue::Double(_)) || matches!(right, ConstantValue::Double(_)) {
        return fold_double(op, double_value(left)?, double_value(right)?);
    }
    if matches!(left, ConstantValue::Float(_)) || matches!(right, ConstantValue::Float(_)) {
        return match fold_double(
            op,
            f64::from(float_value(left)?),
            f64::from(float_value(right)?),
        )? {
            ConstantValue::Double(value) => Some(ConstantValue::Float(value as f32)),
            other => Some(other),
        };
    }
    // Shifts do not promote their right operand into the result type.
    let promotes_to_long =
        matches!(left, ConstantValue::Long(_)) || matches!(right, ConstantValue::Long(_));
    let shift = matches!(op, BinaryOp::Shl | BinaryOp::Shr | BinaryOp::UShr);
    if shift {
        let shift_by = integral_value(right)? as u32;
        return match left {
            ConstantValue::Long(value) => fold_long_shift(op, *value, shift_by % 64),
            _ => fold_int_shift(op, integral_value(left)? as i32, shift_by % 32),
        };
    }
    if promotes_to_long {
        return fold_long(op, integral_value(left)?, integral_value(right)?);
    }
    fold_int(
        op,
        integral_value(left)? as i32,
        integral_value(right)? as i32,
    )
}

fn integral_value(value: &ConstantValue) -> Option<i64> {
    match value {
        ConstantValue::Int(value) => Some(i64::from(*value)),
        ConstantValue::Long(value) => Some(*value),
        ConstantValue::Char(value) => Some(i64::from(*value)),
        _ => None,
    }
}

fn fold_int(op: BinaryOp, left: i32, right: i32) -> Option<ConstantValue> {
    Some(match op {
        BinaryOp::Add => ConstantValue::Int(left.wrapping_add(right)),
        BinaryOp::Sub => ConstantValue::Int(left.wrapping_sub(right)),
        BinaryOp::Mul => ConstantValue::Int(left.wrapping_mul(right)),
        // Division by zero is a compile-time *error* in Java for a constant
        // expression, so refusing to fold is right.
        BinaryOp::Div if right != 0 => ConstantValue::Int(left.wrapping_div(right)),
        BinaryOp::Rem if right != 0 => ConstantValue::Int(left.wrapping_rem(right)),
        BinaryOp::BitAnd => ConstantValue::Int(left & right),
        BinaryOp::BitOr => ConstantValue::Int(left | right),
        BinaryOp::BitXor => ConstantValue::Int(left ^ right),
        BinaryOp::Eq => ConstantValue::Bool(left == right),
        BinaryOp::Ne => ConstantValue::Bool(left != right),
        BinaryOp::Lt => ConstantValue::Bool(left < right),
        BinaryOp::Le => ConstantValue::Bool(left <= right),
        BinaryOp::Gt => ConstantValue::Bool(left > right),
        BinaryOp::Ge => ConstantValue::Bool(left >= right),
        _ => return None,
    })
}

fn fold_long(op: BinaryOp, left: i64, right: i64) -> Option<ConstantValue> {
    Some(match op {
        BinaryOp::Add => ConstantValue::Long(left.wrapping_add(right)),
        BinaryOp::Sub => ConstantValue::Long(left.wrapping_sub(right)),
        BinaryOp::Mul => ConstantValue::Long(left.wrapping_mul(right)),
        BinaryOp::Div if right != 0 => ConstantValue::Long(left.wrapping_div(right)),
        BinaryOp::Rem if right != 0 => ConstantValue::Long(left.wrapping_rem(right)),
        BinaryOp::BitAnd => ConstantValue::Long(left & right),
        BinaryOp::BitOr => ConstantValue::Long(left | right),
        BinaryOp::BitXor => ConstantValue::Long(left ^ right),
        BinaryOp::Eq => ConstantValue::Bool(left == right),
        BinaryOp::Ne => ConstantValue::Bool(left != right),
        BinaryOp::Lt => ConstantValue::Bool(left < right),
        BinaryOp::Le => ConstantValue::Bool(left <= right),
        BinaryOp::Gt => ConstantValue::Bool(left > right),
        BinaryOp::Ge => ConstantValue::Bool(left >= right),
        _ => return None,
    })
}

fn fold_int_shift(op: BinaryOp, left: i32, shift_by: u32) -> Option<ConstantValue> {
    Some(match op {
        BinaryOp::Shl => ConstantValue::Int(left.wrapping_shl(shift_by)),
        BinaryOp::Shr => ConstantValue::Int(left.wrapping_shr(shift_by)),
        // Java's `>>>` is a *logical* shift; doing it arithmetically is its own
        // defect (0005), so it is spelled out here rather than reused.
        BinaryOp::UShr => ConstantValue::Int(((left as u32).wrapping_shr(shift_by)) as i32),
        _ => return None,
    })
}

fn fold_long_shift(op: BinaryOp, left: i64, shift_by: u32) -> Option<ConstantValue> {
    Some(match op {
        BinaryOp::Shl => ConstantValue::Long(left.wrapping_shl(shift_by)),
        BinaryOp::Shr => ConstantValue::Long(left.wrapping_shr(shift_by)),
        BinaryOp::UShr => ConstantValue::Long(((left as u64).wrapping_shr(shift_by)) as i64),
        _ => return None,
    })
}

fn fold_double(op: BinaryOp, left: f64, right: f64) -> Option<ConstantValue> {
    Some(match op {
        BinaryOp::Add => ConstantValue::Double(left + right),
        BinaryOp::Sub => ConstantValue::Double(left - right),
        BinaryOp::Mul => ConstantValue::Double(left * right),
        // Unlike integer division, floating division by zero is defined.
        BinaryOp::Div => ConstantValue::Double(left / right),
        BinaryOp::Rem => ConstantValue::Double(left % right),
        BinaryOp::Eq => ConstantValue::Bool(left == right),
        BinaryOp::Ne => ConstantValue::Bool(left != right),
        BinaryOp::Lt => ConstantValue::Bool(left < right),
        BinaryOp::Le => ConstantValue::Bool(left <= right),
        BinaryOp::Gt => ConstantValue::Bool(left > right),
        BinaryOp::Ge => ConstantValue::Bool(left >= right),
        _ => return None,
    })
}

/// How Java renders a constant inside a string concatenation.
fn java_display(value: &ConstantValue) -> Option<Vec<u16>> {
    let scalar = match value {
        ConstantValue::String(value) => return Some(value.clone()),
        ConstantValue::Bool(value) => value.to_string(),
        ConstantValue::Int(value) => value.to_string(),
        ConstantValue::Long(value) => value.to_string(),
        ConstantValue::Char(value) => return Some(vec![*value]),
        // Java's float/double `toString` is not Rust's, and a constant whose
        // text has to be exactly right is not worth guessing at.
        ConstantValue::Float(_) | ConstantValue::Double(_) => return None,
    };
    Some(scalar.encode_utf16().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java_ir::Member;

    /// Folds the single `static final` field of `source`, at its declared type.
    fn fold_declared(source: &str) -> Option<String> {
        let parsed = crate::parser::parse_java_source(source);
        let mut scope = ConstantScope::new();
        let mut own = BTreeMap::new();
        let mut last = None;
        for ty in &parsed.program.types {
            for member in &ty.members {
                let Member::Field(field) = member else {
                    continue;
                };
                let initializer = field.initializer.as_ref()?;
                let value = fold_constant_expr(initializer, &scope)?;
                last = value.rust_value_for(&field.ty);
                own.insert(field.name.clone(), value.clone());
                own.insert(crate::parser::to_snake_case(&field.name), value);
                scope.focus_on_owner(None, own.clone());
            }
        }
        last
    }

    fn fold_one(declaration: &str) -> Option<String> {
        fold_declared(&format!("public class C {{ {declaration} }}"))
    }

    #[test]
    fn folds_integral_literals_and_arithmetic() {
        assert_eq!(fold_one("static final int A = 9;").as_deref(), Some("9"));
        assert_eq!(fold_one("static final int A = -1;").as_deref(), Some("-1"));
        assert_eq!(
            fold_one("static final int A = 2 * 3 + 4;").as_deref(),
            Some("10")
        );
        assert_eq!(
            fold_one("static final long A = 1L << 40;").as_deref(),
            Some("1099511627776i64")
        );
        // An `int` constant widens to `long` without a cast in Java.
        assert_eq!(
            fold_one("static final long A = 7;").as_deref(),
            Some("7i64")
        );
    }

    #[test]
    fn unsigned_right_shift_is_logical_not_arithmetic() {
        // The defect 0005 is about, in constant position.
        assert_eq!(
            fold_one("static final int A = -1 >>> 28;").as_deref(),
            Some("15")
        );
        assert_eq!(
            fold_one("static final int A = -1 >> 28;").as_deref(),
            Some("-1")
        );
    }

    #[test]
    fn int_arithmetic_wraps_at_java_width() {
        assert_eq!(
            fold_one("static final int A = 2147483647 + 1;").as_deref(),
            Some("-2147483648")
        );
    }

    #[test]
    fn folds_floating_values_including_the_ones_with_no_literal_form() {
        assert_eq!(
            fold_one("static final double A = -1.0;").as_deref(),
            Some("-1.0")
        );
        // `1.0` must not render as `1`, which is not an f64 literal.
        assert_eq!(
            fold_one("static final double A = 0.5 + 0.5;").as_deref(),
            Some("1.0")
        );
        assert_eq!(
            fold_one("static final double A = 1.0 / 0.0;").as_deref(),
            Some("f64::INFINITY")
        );
        assert_eq!(
            fold_one("static final float A = 1.5f;").as_deref(),
            Some("1.5f32")
        );
    }

    #[test]
    fn folds_strings_and_their_constant_concatenation() {
        assert_eq!(
            fold_one("static final String A = \"BCF\";").as_deref(),
            Some("\"BCF\"")
        );
        assert_eq!(
            fold_one("static final String A = \"a\" + 1 + true;").as_deref(),
            Some("\"a1true\"")
        );
    }

    #[test]
    fn folds_references_to_earlier_constants_of_the_same_owner() {
        assert_eq!(
            fold_one("static final int A = 9; static final int B = A + 1;").as_deref(),
            Some("10")
        );
    }

    #[test]
    fn refuses_anything_that_is_not_a_constant_expression() {
        // A call: the value is genuinely unknown, and 0031's whole argument is
        // that it must stay unknown rather than become a plausible zero.
        assert_eq!(fold_one("static final int A = compute();"), None);
        // A reference to another class's field.
        assert_eq!(fold_one("static final int A = Other.B;"), None);
        // `null` is not a constant expression in the JLS sense.
        assert_eq!(fold_one("static final String A = null;"), None);
        // Integer division by zero is a compile-time error in Java, so folding
        // it to anything at all would be inventing a value.
        assert_eq!(fold_one("static final int A = 1 / 0;"), None);
    }

    #[test]
    fn a_declared_type_that_disagrees_with_the_folded_value_yields_nothing() {
        // Folding must never emit a value the declared type cannot hold; a
        // mismatch means the fold itself was wrong.
        assert_eq!(
            ConstantValue::Int(300).rust_value_for(&JavaType::Primitive(PrimitiveType::Byte)),
            None
        );
        assert_eq!(
            ConstantValue::Bool(true).rust_value_for(&JavaType::Primitive(PrimitiveType::Int)),
            None
        );
        assert_eq!(
            ConstantValue::String(vec![120])
                .rust_value_for(&JavaType::Primitive(PrimitiveType::Int)),
            None
        );
    }

    #[test]
    fn folds_casts_the_way_java_narrows_them() {
        assert_eq!(
            fold_one("static final byte A = (byte) 300;").as_deref(),
            Some("44")
        );
        assert_eq!(
            fold_one("static final int A = (int) 3.9;").as_deref(),
            Some("3")
        );
        assert_eq!(
            fold_one("static final char A = (char) 65;").as_deref(),
            Some("'A'")
        );
    }

    #[test]
    fn constant_char_narrowing_wraps_and_numeric_promotion_keeps_surrogates() {
        for (expression, expected) in [
            ("(int) (char) -1", "65535"),
            ("(int) (char) 65536", "0"),
            ("(int) (char) 128512", "62976"),
            ("(int) (char) 1e100", "65535"),
            ("(int) (char) -1e100", "0"),
            (r"'\ud83d' + 1", "55358"),
            (r"+'\ud83d'", "55357"),
            (r"-'\ud83d'", "-55357"),
            (r"~'\ud83d'", "-55358"),
        ] {
            assert_eq!(
                fold_one(&format!("static final int A = {expression};")).as_deref(),
                Some(expected),
                "{expression}"
            );
        }
        assert_eq!(
            fold_one(r"static final double A = (double) '\ud83d';").as_deref(),
            Some("55357.0")
        );
    }

    #[test]
    fn constant_concatenation_preserves_units_and_can_recombine_surrogates() {
        let expr = crate::parser::parse_expr(r#""\ud83d" + '\ude00'"#);
        assert_eq!(
            fold_constant_expr(&expr, &ConstantScope::new()),
            Some(ConstantValue::String(vec![0xd83d, 0xde00]))
        );
        assert_eq!(
            fold_one(r#"static final String A = "\ud83d" + '\ude00';"#).as_deref(),
            Some("\"😀\"")
        );
        let expr = crate::parser::parse_expr(r#""x" + '\ud83d' + 1"#);
        let value = fold_constant_expr(&expr, &ConstantScope::new()).unwrap();
        assert_eq!(value, ConstantValue::String(vec![120, 0xd83d, 49]));
        // A scalar-only facade cannot render this value. Preserve it in the
        // fold, but refuse emission instead of manufacturing scalar text.
        assert_eq!(value.rust_value_for(&JavaType::String), None);
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;
    use crate::java_ir::PrimitiveType;

    fn int(value: i32) -> ConstantValue {
        ConstantValue::Int(value)
    }

    fn folded(scope: &ConstantScope, path: &str) -> Option<i32> {
        match scope.lookup_path(path) {
            Some(ConstantValue::Int(value)) => Some(value),
            _ => None,
        }
    }

    /// Two packages declaring a class of the same simple name must not share
    /// its constants. Recording simple owner names in one flat map made
    /// `pkg1.Uses.Q = Constants.X + 1` fold to **23** using `pkg2`'s `X = 22`
    /// instead of 12 using `pkg1`'s `X = 11` — a confidently wrong number,
    /// which is the failure
    /// [0031](../claude_audits/done/0031-stubbed-constant-census.md) is about.
    #[test]
    fn a_simple_owner_name_resolves_within_the_referring_package() {
        let mut scope = ConstantScope::new();
        scope.record("pkg1.Constants", "X", int(11));
        scope.record("pkg2.Constants", "X", int(22));

        scope.focus_on_owner(Some("pkg1"), BTreeMap::new());
        assert_eq!(folded(&scope, "Constants.X"), Some(11));

        scope.focus_on_owner(Some("pkg2"), BTreeMap::new());
        assert_eq!(folded(&scope, "Constants.X"), Some(22));
    }

    /// A fully-qualified path is never affected by which package is referring.
    #[test]
    fn a_qualified_path_wins_over_package_scoping() {
        let mut scope = ConstantScope::new();
        scope.record("pkg1.Constants", "X", int(11));
        scope.record("pkg2.Constants", "X", int(22));

        scope.focus_on_owner(Some("pkg1"), BTreeMap::new());
        assert_eq!(folded(&scope, "pkg2.Constants.X"), Some(22));
        assert_eq!(folded(&scope, "pkg1.Constants.X"), Some(11));
    }

    /// An ambiguous simple name with no package to disambiguate it must fold to
    /// nothing, so the loud `compile_error!` stands. Picking either value would
    /// be a coin flip presented as an answer.
    #[test]
    fn an_ambiguous_simple_owner_refuses_to_fold() {
        let mut scope = ConstantScope::new();
        scope.record("pkg1.Constants", "X", int(11));
        scope.record("pkg2.Constants", "X", int(22));

        scope.focus_on_owner(Some("pkg3"), BTreeMap::new());
        assert_eq!(folded(&scope, "Constants.X"), None);

        scope.focus_on_owner(None, BTreeMap::new());
        assert_eq!(folded(&scope, "Constants.X"), None);
    }

    /// One owner with that simple name is unambiguous wherever it is referred
    /// from — the common case, and it must keep working.
    #[test]
    fn a_unique_simple_owner_resolves_from_any_package() {
        let mut scope = ConstantScope::new();
        scope.record("pkg1.Constants", "X", int(11));

        scope.focus_on_owner(Some("elsewhere"), BTreeMap::new());
        assert_eq!(folded(&scope, "Constants.X"), Some(11));
    }

    /// Bare names are the *focused* owner's own constants and must not leak.
    #[test]
    fn bare_names_belong_to_the_focused_owner_only() {
        let mut scope = ConstantScope::new();
        scope.record("pkg1.Constants", "X", int(11));

        let mut own = BTreeMap::new();
        own.insert("X".to_string(), int(11));
        scope.focus_on_owner(Some("pkg1"), own);
        assert_eq!(scope.lookup_bare("X"), Some(int(11)));

        scope.focus_on_owner(Some("pkg2"), BTreeMap::new());
        assert_eq!(scope.lookup_bare("X"), None);
    }

    /// The snake-cased spelling the parser produces resolves the same way.
    #[test]
    fn snake_cased_reference_spellings_resolve_identically() {
        let mut scope = ConstantScope::new();
        scope.record("pkg1.BcfFormat", "SIZEOF_BCF_HEADER", int(9));

        scope.focus_on_owner(Some("pkg1"), BTreeMap::new());
        assert_eq!(folded(&scope, "bcf_format.SIZEOF_BCF_HEADER"), Some(9));
        assert_eq!(folded(&scope, "BcfFormat.SIZEOF_BCF_HEADER"), Some(9));

        // And the declared type still gates what is emitted.
        assert_eq!(
            int(9).rust_value_for(&JavaType::Primitive(PrimitiveType::Int)),
            Some("9".to_string())
        );
    }
}
