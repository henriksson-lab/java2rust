//! Admission for erased Object[] operation captures, including backed-list
//! factories. Stored Java array slots require their own signature/storage
//! contract; these operation views do not relabel a Vec.
use crate::expr_shape::{ExprShapeContext, ShapeEnv, infer_expr_shape_with_context};
use crate::java_ir::{Expr, JavaType, Literal};
use crate::slot_shape::DispatchShape;

pub(crate) fn erased_array_cast_source<'a>(
    expr: &'a Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<&'a Expr> {
    let Expr::Cast {
        ty: JavaType::Array(element),
        expr: source,
    } = expr
    else {
        return None;
    };
    let JavaType::Named { name, args } = element.as_ref() else {
        return None;
    };
    if !args.is_empty()
        || !matches!(name.as_str(), "Object" | "java.lang.Object")
        || !crate::effective_facts::EffectiveFacts::new(ctx.facts()?)
            .allows_platform_hierarchy_fallback(name)
    {
        return None;
    }
    (matches!(source.as_ref(), Expr::Literal(Literal::Null))
        || infer_expr_shape_with_context(source, env, ctx).dispatch == DispatchShape::ErasedObject)
        .then_some(source)
}

/// Java Object[] storage must accept typed reference-array owners without
/// changing their allocation or component class. The operation carrier itself
/// is local; native allocation qualifiers describe its backing, not Send/Sync.
pub(crate) fn select_stored_view(
    shape: &mut crate::slot_shape::RustSlotShape,
    facts: &crate::facts::FactSet,
    type_params: &[String],
) {
    let base = shape
        .optional_element_type()
        .unwrap_or_else(|| shape.java_ty.clone());
    let JavaType::Array(element) = base else {
        return;
    };
    let element = crate::types::optional_element_type(&element).unwrap_or(*element);
    let JavaType::Named { name, args } = element else {
        return;
    };
    if args.is_empty()
        && matches!(name.as_str(), "Object" | "java.lang.Object")
        && !type_params.contains(&name)
        && crate::effective_facts::EffectiveFacts::new(facts)
            .allows_platform_hierarchy_fallback(&name)
    {
        shape.view = crate::slot_shape::ViewShape::ReferenceArray;
        shape.ownership = crate::slot_shape::OwnershipShape::Owned;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slot_shape::{RustSlotShape, ViewShape};
    #[test]
    fn null_generic_array_results_keep_nullable_local_storage() {
        let parsed = crate::parser::parse_java_source(include_str!(
            "../tests/corpus/csv/StoredArrayViewContracts.java"
        ));
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &crate::facts::FactSet::default(),
        );
        let representations =
            crate::representation::plan_representations(&resolved.program, &semantic.facts);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &semantic.usage,
            &semantic.facts,
        );
        let owner = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "StoredArrayViewContracts")
            .unwrap();
        let method = owner
            .members
            .iter()
            .find_map(|member| match member {
                crate::java_ir::Member::Method(method) if method.name == "run" => Some(method),
                _ => None,
            })
            .unwrap();
        let signature = signatures.method_signature(owner, method).unwrap();
        let local = signature
            .locals()
            .iter()
            .find(|local| local.name() == "generic_null")
            .unwrap();
        assert!(
            local.shape().optional_element_type().is_some(),
            "{local:#?}"
        );
    }

    #[test]
    fn stored_array_selection_preserves_source_and_binder_authority() {
        let parsed = crate::parser::parse_java_source(
            "class Object {} class Uses { void source(Object[] values) {} void platform(java.lang.Object[] values) {} }",
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        for (ty, scope, expected) in [
            ("Object[]", vec![], ViewShape::Native),
            ("java.lang.Object[]", vec![], ViewShape::ReferenceArray),
        ] {
            let mut shape = RustSlotShape::for_storage(crate::parser::parse_hierarchy_type(ty));
            select_stored_view(&mut shape, &resolved.facts, &scope);
            assert_eq!(shape.view, expected, "{ty}");
        }
        let mut binder =
            RustSlotShape::for_storage(crate::parser::parse_hierarchy_type("Object[]"));
        select_stored_view(
            &mut binder,
            &crate::facts::FactSet::default(),
            &["Object".into()],
        );
        assert_eq!(binder.view, ViewShape::Native);
    }
}
