use crate::java_ir::{BinaryOp, Literal};
use crate::literal_lowering::literal_to_rust;
use crate::operator_lowering::binary_op_to_rust;
use crate::rust_ir::RustExpr;

pub fn named_constructor_expr(
    type_name: &str,
    constructor_name: &str,
    args: Vec<RustExpr>,
) -> RustExpr {
    let owner = crate::java_naming::rust_name_for_java_named_type(type_name);
    let function = crate::coercion::owner_associated_path_projection(&owner, constructor_name);
    crate::coercion::function_call_projection(function, args)
}

pub fn this_expr() -> RustExpr {
    RustExpr::Var("self".to_string())
}

pub fn super_expr() -> RustExpr {
    crate::unsupported::compile_error_expr("unsupported standalone Java super expression")
}

pub fn literal_expr(lit: &Literal) -> RustExpr {
    RustExpr::Literal(literal_to_rust(lit))
}

pub fn indexed_expr(array: RustExpr, index: RustExpr) -> RustExpr {
    RustExpr::Index(Box::new(array), Box::new(index))
}

pub fn binary_expr(op: BinaryOp, left: RustExpr, right: RustExpr) -> RustExpr {
    if op == BinaryOp::UShr {
        return crate::unsupported::compile_error_expr(
            "unsupported Java unsigned right shift without numeric type context",
        );
    }
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
