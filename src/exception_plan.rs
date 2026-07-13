use crate::java_ir::{Expr, JavaType, Program};

#[derive(Debug, Clone, PartialEq)]
pub enum ThrowExprPlan<'a> {
    NewException {
        class_name: String,
        args: &'a [Expr],
    },
    FallbackValue(&'a Expr),
}

pub fn program_uses_java_exception(program: &Program) -> bool {
    program.types.iter().any(type_uses_java_exception)
}

pub fn plan_throw_expr(expr: &Expr) -> ThrowExprPlan<'_> {
    match expr {
        Expr::New { ty, args, .. } => ThrowExprPlan::NewException {
            class_name: exception_class_name(ty),
            args,
        },
        _ => ThrowExprPlan::FallbackValue(expr),
    }
}

pub fn exception_class_name(class_ty: &JavaType) -> String {
    match class_ty {
        JavaType::Named { name, .. } => name.clone(),
        JavaType::Unknown(raw) => raw.clone(),
        _ => "JavaException".to_string(),
    }
}

fn type_uses_java_exception(ty: &crate::java_ir::TypeDecl) -> bool {
    ty.members.iter().any(|member| match member {
        crate::java_ir::Member::Method(method) => !method.throws.is_empty(),
        crate::java_ir::Member::Constructor(constructor) => !constructor.throws.is_empty(),
        crate::java_ir::Member::Type(ty) => type_uses_java_exception(ty),
        _ => false,
    })
}
