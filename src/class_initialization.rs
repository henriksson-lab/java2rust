//! Materialize class-initializer bodies as internal callables so ordinary
//! binding, storage, liveness and exception analysis owns their contracts.
use crate::java_ir::{
    Block, Expr, JavaType, Member, MethodDecl, Modifier, Program, Stmt, TypeDecl,
};

// Not a Java identifier: source declarations cannot impersonate this method.
const INITIALIZER_NAME: &str = "<clinit>";

pub(crate) fn is_initializer(method: &MethodDecl) -> bool {
    method.name == INITIALIZER_NAME
}

pub(crate) fn initializer(ty: &TypeDecl) -> Option<&MethodDecl> {
    ty.members.iter().find_map(|member| match member {
        Member::Method(method) if is_initializer(method) => Some(method),
        _ => None,
    })
}

pub(crate) fn requires_initialization(ty: &TypeDecl) -> bool {
    !ty.static_init_blocks.is_empty()
        || initializer(ty).is_some()
        || crate::constructor_plan::enum_requires_instance_initialization(ty)
}

pub(crate) fn materialize(program: &mut Program) {
    for ty in &mut program.types {
        materialize_type(ty);
    }
}

fn materialize_type(ty: &mut TypeDecl) {
    for member in &mut ty.members {
        if let Member::Type(nested) = member {
            materialize_type(nested);
        }
    }
    if !requires_initialization(ty) || initializer(ty).is_some() {
        return;
    }
    let mut statements = Vec::new();
    // Keep each block's lexical scope while interleaving field writes in the
    // original Java order. Existing associated scalar constants need no store.
    for index in 0..=ty.members.len() {
        for block in &ty.static_init_blocks {
            if block.member_index == index {
                statements.push(Stmt::Block(block.block.clone()));
            }
        }
        let Some(Member::Field(field)) = ty.members.get(index) else {
            continue;
        };
        if !field.modifiers.contains(&Modifier::Static)
            || crate::item_assembly::static_final_field_has_associated_const(ty, field)
            || (field.is_enum_constant()
                && !crate::constructor_plan::enum_requires_instance_initialization(ty))
        {
            continue;
        }
        if let Some(value) = &field.initializer {
            statements.push(Stmt::Assign {
                target: Expr::Var(field.name.clone()),
                value: value.clone(),
            });
        }
    }
    ty.members.push(Member::Method(MethodDecl {
        name: INITIALIZER_NAME.into(),
        rust_name: Some("__java_class_init_body".into()),
        type_params: Vec::new(),
        type_param_bounds: Vec::new(),
        return_type: JavaType::Void,
        throws: Vec::new(),
        params: Vec::new(),
        modifiers: vec![Modifier::Private, Modifier::Static],
        body: Some(Block { statements }),
        span: ty.span,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_typed_enum_aliases_remain_ordinary_ordered_static_fields() {
        let mut program = crate::parser::parse_java_source(
            "enum E { FIRST; static final E ALIAS = FIRST; static E chosen = FIRST; static { chosen = ALIAS; } }",
        ).program;
        materialize(&mut program);
        let body = initializer(&program.types[0])
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        assert_eq!(body.statements.len(), 3);
        assert!(
            matches!(&body.statements[0], Stmt::Assign { target: Expr::Var(name), .. } if name == "ALIAS")
        );
        assert!(
            matches!(&body.statements[1], Stmt::Assign { target: Expr::Var(name), .. } if name == "chosen")
        );
        assert!(matches!(&body.statements[2], Stmt::Block(_)));
    }

    #[test]
    fn initializer_is_ordered_scoped_and_idempotent() {
        let mut program = crate::parser::parse_java_source(
            "class Init { static int a = first(); static { int x = a; } static int b = second(); static { int x = b; } static int first() { return 1; } static int second() { return 2; } }",
        ).program;
        materialize(&mut program);
        let first = program.clone();
        materialize(&mut program);
        assert_eq!(program, first);
        let body = initializer(&program.types[0])
            .unwrap()
            .body
            .as_ref()
            .unwrap();
        assert_eq!(body.statements.len(), 4);
        assert!(
            matches!(&body.statements[0], Stmt::Assign { target: Expr::Var(name), .. } if name == "a")
        );
        assert!(matches!(&body.statements[1], Stmt::Block(_)));
        assert!(
            matches!(&body.statements[2], Stmt::Assign { target: Expr::Var(name), .. } if name == "b")
        );
        assert!(matches!(&body.statements[3], Stmt::Block(_)));
    }
}
