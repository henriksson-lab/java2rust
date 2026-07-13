use crate::java_ir::{BinaryOp, UnaryOp};
use crate::rust_ir::RustExpr;

pub fn binary_op_to_rust(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
    }
}

pub fn lower_unary_expr(op: UnaryOp, operand: RustExpr) -> RustExpr {
    match op {
        UnaryOp::PostInc => RustExpr::Raw(format!("{} += 1", crate::emit::emit_expr(&operand))),
        UnaryOp::PostDec => RustExpr::Raw(format!("{} -= 1", crate::emit::emit_expr(&operand))),
        UnaryOp::Not => RustExpr::Unary {
            op: "!".to_string(),
            expr: Box::new(operand),
        },
        UnaryOp::Neg => RustExpr::Unary {
            op: "-".to_string(),
            expr: Box::new(operand),
        },
        UnaryOp::Pos => operand,
        UnaryOp::PreInc | UnaryOp::PreDec => RustExpr::Raw(crate::emit::emit_expr(&operand)),
    }
}

pub fn stream_unary_prefix(op: UnaryOp) -> Option<&'static str> {
    match op {
        UnaryOp::Not => Some("!"),
        UnaryOp::Neg => Some("-"),
        UnaryOp::Pos => Some(""),
        _ => None,
    }
}
