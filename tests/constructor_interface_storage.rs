use new_java2rust::{
    java_ir::Member,
    parser::parse_java_source,
    representation::plan_representations,
    resolve::resolve_program,
    semantic_analysis::analyze_program,
    signature::{
        SignatureConstraint, SignatureShapeRequest, SignatureSlotRole,
        solve_signature_shape_with_facts, solve_signatures_with_facts,
    },
    slot_shape::{DispatchShape, OwnershipShape, ViewShape},
    translate_source,
};

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_test_generated_modules};

const SOURCE: &str = r#"
interface SourceNodePort<T> { int size(); }
class CountPort implements SourceNodePort<String> {
    public int size() { return 7; }
}
class PortRetainer<T> {
    SourceNodePort<T> port;
    public PortRetainer(SourceNodePort<T> port) { this.port = port; }
    SourceNodePort<T> result() { return port; }
}
class RetainerUse {
    static SourceNodePort<String> forward(SourceNodePort<String> input) {
        return new PortRetainer<>(input).result();
    }
    static int probe() { return forward(new CountPort()).size(); }
}
"#;

#[test]
fn constructor_retained_interface_and_field_keep_one_storage_contract() {
    let parsed = parse_java_source(SOURCE);
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let external = new_java2rust::stdlib::rust_stdlib_signature_facts();
    let semantic = analyze_program(&resolved.program, &resolved.facts, &external);
    let representations = plan_representations(&resolved.program, &semantic.facts);
    let plan = solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &semantic.usage,
        &semantic.facts,
    );
    let owner = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "PortRetainer")
        .unwrap();
    let constructor = owner
        .members
        .iter()
        .find_map(|member| match member {
            Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();
    let param = &plan.constructor_param_plans(owner, constructor).unwrap()[0];
    assert!(
        matches!(param.shape().dispatch, DispatchShape::BoxedDynTrait { .. }),
        "{param:?}"
    );
    assert_eq!(param.shape().view, ViewShape::DynTraitView);
    assert_eq!(param.shape().ownership, OwnershipShape::Owned);
    let field = owner
        .members
        .iter()
        .find_map(|member| match member {
            Member::Field(field) if field.name == "port" => Some(field),
            _ => None,
        })
        .unwrap();
    let field_shape = plan.field_shape_for_declared_field(owner, field).unwrap();
    assert_eq!(field_shape.dispatch, param.shape().dispatch);
    assert_eq!(field_shape.view, param.shape().view);
    assert_eq!(field_shape.ownership, param.shape().ownership);
    let translation = translate_source(SOURCE);
    assert!(
        !translation
            .rust
            .contains("unsupported Java dispatch coercion"),
        "{}",
        translation
            .rust
            .lines()
            .filter(|line| line.contains("fn new(") || line.contains("compile_error!"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    cargo_test_generated_modules(
        "retained_source_interface",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn retained_interface_preserves_behavior() {
    assert_eq!(RetainerUse::probe(), 7);
}
"#,
        }],
    );
}

#[test]
fn source_interface_slot_roles_share_authoritative_dispatch_without_erasing_object_boundaries() {
    use new_java2rust::java_ir::JavaType;
    let parsed = parse_java_source("interface Port<Element> { int size(); }");
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = analyze_program(&resolved.program, &resolved.facts, &Default::default());
    let reps = plan_representations(&resolved.program, &semantic.facts);
    let port = JavaType::Named {
        name: "Port".into(),
        args: vec![JavaType::String],
    };
    for (role, stored) in [
        (SignatureSlotRole::Param { is_varargs: false }, false),
        (SignatureSlotRole::Param { is_varargs: false }, true),
        (SignatureSlotRole::Return, false),
        (SignatureSlotRole::Storage, false),
    ] {
        for nullable in [false, true] {
            let java_ty = if nullable {
                new_java2rust::types::optional_java_type(port.clone())
            } else {
                port.clone()
            };
            let mut constraints = vec![
                SignatureConstraint::RequiredDispatch(DispatchShape::ErasedObject),
                SignatureConstraint::RequiredView(ViewShape::ErasedObjectView),
            ];
            if stored {
                constraints.push(SignatureConstraint::StorageSlot);
            }
            let shape = solve_signature_shape_with_facts(
                &reps,
                SignatureShapeRequest::new(java_ty.clone(), role, constraints, vec![]),
                &semantic.facts,
            )
            .unwrap();
            let expected = if stored || !matches!(role, SignatureSlotRole::Param { .. }) {
                DispatchShape::BoxedDynTrait {
                    trait_name: "Port".into(),
                }
            } else {
                DispatchShape::DynTrait {
                    trait_name: "Port".into(),
                }
            };
            assert_eq!(shape.dispatch, expected, "{role:?}, nullable={nullable}");
            assert_eq!(shape.view, ViewShape::DynTraitView);
            assert_eq!(shape.java_ty, java_ty);
        }
    }
    let object = JavaType::Unknown("Object".into());
    let shape = solve_signature_shape_with_facts(
        &reps,
        SignatureShapeRequest::new(object, SignatureSlotRole::Storage, vec![], vec![]),
        &semantic.facts,
    )
    .unwrap();
    assert_eq!(shape.dispatch, DispatchShape::ErasedObject);
    assert_eq!(shape.view, ViewShape::ErasedObjectView);

    // A deliberately erased source representation remains an explicit ABI
    // choice; source capability alone must not invent a typed trait carrier.
    let mut facts = semantic.facts.clone();
    facts
        .hierarchy_overrides
        .push(new_java2rust::facts::HierarchyOverride {
            type_name: "Port".into(),
            strategy: new_java2rust::facts::HierarchyStrategy::ErasedObject,
            provenance: new_java2rust::facts::FactProvenance::UserOverride,
        });
    let reps = plan_representations(&resolved.program, &facts);
    let shape = solve_signature_shape_with_facts(
        &reps,
        SignatureShapeRequest::new(port, SignatureSlotRole::Storage, vec![], vec![]),
        &facts,
    )
    .unwrap();
    assert_eq!(shape.dispatch, DispatchShape::ErasedObject);
    assert_eq!(shape.view, ViewShape::ErasedObjectView);
}

#[test]
fn generic_field_access_specializes_declared_parameter_names_and_preserves_nullability() {
    use new_java2rust::{
        expr_shape::{ExprShapeContext, ShapeEnv, infer_expr_shape_with_context},
        java_ir::{Expr, JavaType},
    };
    let parsed = parse_java_source(
        "interface ItemView<Element> {} class Holder<Value> { ItemView<Value> item; }",
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let semantic = analyze_program(&resolved.program, &resolved.facts, &Default::default());
    let reps = plan_representations(&resolved.program, &semantic.facts);
    let signatures =
        solve_signatures_with_facts(&resolved.program, &reps, &semantic.usage, &semantic.facts);
    let mut env = ShapeEnv::new();
    env.insert_local_shape(
        "holder",
        new_java2rust::slot_shape::RustSlotShape::for_storage(JavaType::Named {
            name: "Holder".into(),
            args: vec![JavaType::String],
        }),
    );
    let shape = infer_expr_shape_with_context(
        &Expr::FieldAccess(Box::new(Expr::Var("holder".into())), "item".into()),
        &env,
        ExprShapeContext::new(Some(&signatures), Some(&reps), Some(&semantic.facts)),
    );
    let element = shape
        .optional_element_type()
        .expect("default-null field must retain its null state");
    assert_eq!(
        element,
        JavaType::Named {
            name: "ItemView".into(),
            args: vec![JavaType::String]
        }
    );
    assert!(matches!(
        shape.dispatch,
        DispatchShape::BoxedDynTrait { .. }
    ));
}
