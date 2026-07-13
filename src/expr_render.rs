use crate::java_ir::{BinaryOp, Literal};
use crate::literal_lowering::literal_to_rust;
use crate::operator_lowering::binary_op_to_rust;
use crate::rust_ir::RustExpr;

pub fn this_expr() -> RustExpr {
    RustExpr::Var("self".to_string())
}

pub fn super_expr() -> RustExpr {
    RustExpr::Raw("super".to_string())
}

pub fn raw_expr(raw: &str) -> RustExpr {
    RustExpr::Raw(raw.to_string())
}

pub fn literal_expr(lit: &Literal) -> RustExpr {
    RustExpr::Literal(literal_to_rust(lit))
}

pub fn indexed_expr(array: RustExpr, index: RustExpr) -> RustExpr {
    RustExpr::Index(Box::new(array), Box::new(index))
}

pub fn binary_expr(op: BinaryOp, left: RustExpr, right: RustExpr) -> RustExpr {
    RustExpr::Binary {
        op: binary_op_to_rust(op).to_string(),
        left: Box::new(left),
        right: Box::new(right),
    }
}

pub fn ternary_expr(condition: RustExpr, then_expr: RustExpr, else_expr: RustExpr) -> RustExpr {
    RustExpr::If {
        condition: Box::new(condition),
        then_expr: Box::new(then_expr),
        else_expr: Box::new(else_expr),
    }
}
