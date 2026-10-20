use new_java2rust::{
    facts::FactSet, java_ir::Member, parser, representation, resolve, semantic_analysis, signature,
    slot_shape::DispatchShape,
};

#[test]
fn cast_value_domains_do_not_copy_object_erasure_into_generic_results() {
    let parsed = parser::parse_java_source(include_str!("fixtures/nullable_reference_casts.java"));
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    assert!(!resolved.diagnostics.has_errors());
    let semantic =
        semantic_analysis::analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
    let representations = representation::plan_representations(&resolved.program, &semantic.facts);
    let plan = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let owner = &resolved.program.types[0];
    for member in &owner.members {
        let Member::Method(method) = member else {
            continue;
        };
        if [
            "cast",
            "throughLocal",
            "assignedLocal",
            "choose",
            "element",
            "nestedElement",
        ]
        .contains(&method.name.as_str())
        {
            let shape = plan
                .required_method_signature(owner, method)
                .return_shape()
                .unwrap();
            assert!(
                shape.optional_element_type().is_some(),
                "{}: {shape:?}",
                method.name
            );
            assert_eq!(
                shape.dispatch,
                DispatchShape::Concrete,
                "{}: {shape:?}",
                method.name
            );
            assert!(!shape.native_allocation.shared());
        }
        if method.name == "primitiveResult" {
            assert!(
                plan.required_method_signature(owner, method)
                    .return_shape()
                    .unwrap()
                    .optional_element_type()
                    .is_none()
            );
        }
    }
}
