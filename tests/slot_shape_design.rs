use new_java2rust::facts::{FactProvenance, FactSet, HierarchyOverride, HierarchyStrategy};
use new_java2rust::java_ir::{Expr, JavaType, Member, PrimitiveType, Stmt};
use new_java2rust::lower::lower_program_with_plans;
use new_java2rust::parser::{parse_expr, parse_java_source};
use new_java2rust::representation::{
    ObjectRepresentation, RepresentationProvenance, RepresentationTypeEmission,
    plan_representations,
};
use new_java2rust::resolve::resolve_program;
use new_java2rust::rust_ir::{RustExpr, RustType};
use new_java2rust::signature::{ReceiverPlan, solve_signatures};
use new_java2rust::slot_shape::{
    DispatchShape, LoweredType, MutabilityShape, OwnershipShape, RustSlotShape, TypeCapabilities,
    ViewShape, list_java_type, optional_java_type,
};
use new_java2rust::types::{
    ExprShapeContext, TypeEnv, infer_assignment_target_shape,
    infer_assignment_target_shape_with_context, infer_expr_shape, infer_expr_shape_with_context,
    infer_stream_lambda_body_shape, infer_stream_map_to_int_shape,
    infer_stream_source_element_shape,
};
use new_java2rust::usage::{BindingUse, analyze_usage};
use new_java2rust::{emit, translate_source};

#[test]
fn slot_shape_keeps_string_ownership_and_view_as_separate_axes() {
    let mut shape = RustSlotShape::conservative(JavaType::String);
    assert_eq!(shape.ownership, OwnershipShape::Owned);
    assert_eq!(shape.view, ViewShape::Native);
    assert_eq!(shape.rust_type(), RustType::String);

    shape.ownership = OwnershipShape::SharedBorrow;
    shape.view = ViewShape::Str;
    assert_eq!(
        shape.rust_type(),
        RustType::Ref {
            mutable: false,
            inner: Box::new(RustType::Raw("str".to_string()))
        }
    );
}

#[test]
fn slot_shape_projects_arrays_and_primitives_without_type_specific_ownership_variants() {
    let int_shape = RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    assert_eq!(int_shape.ownership, OwnershipShape::CopyValue);
    assert_eq!(int_shape.view, ViewShape::Native);
    assert_eq!(int_shape.rust_type(), RustType::I32);

    let mut array_shape = RustSlotShape::conservative(JavaType::Array(Box::new(JavaType::String)));
    array_shape.ownership = OwnershipShape::SharedBorrow;
    array_shape.view = ViewShape::Slice;
    assert_eq!(
        array_shape.rust_type(),
        RustType::Ref {
            mutable: false,
            inner: Box::new(RustType::Slice(Box::new(RustType::String)))
        }
    );
}

#[test]
fn slot_shape_owns_storage_defaults() {
    let storage = RustSlotShape::for_storage(JavaType::String);
    assert_eq!(storage.ownership, OwnershipShape::Owned);
    assert_eq!(storage.view, ViewShape::Native);
    assert_eq!(storage.mutability, MutabilityShape::Mutable);

    let primitive_storage = RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Boolean));
    assert_eq!(primitive_storage.ownership, OwnershipShape::CopyValue);
    assert_eq!(primitive_storage.mutability, MutabilityShape::Mutable);
}

#[test]
fn type_capabilities_drive_copy_string_and_sequence_facts() {
    let string_caps = TypeCapabilities::conservative(&JavaType::String);
    assert!(string_caps.string_like);
    assert!(string_caps.clone);
    assert!(!string_caps.copy);

    let int_caps = TypeCapabilities::conservative(&JavaType::Primitive(PrimitiveType::Int));
    assert!(int_caps.copy);

    let list_ty = list_java_type(JavaType::String);
    let list_caps = TypeCapabilities::conservative(&list_ty);
    assert_eq!(list_caps.sequence_like, Some(JavaType::String));
    assert_eq!(
        RustSlotShape::conservative(list_ty)
            .capabilities()
            .sequence_like,
        Some(JavaType::String)
    );
}

#[test]
fn slot_shape_owns_collection_and_optional_queries() {
    let string_shape = RustSlotShape::conservative(JavaType::String);
    let list_shape = RustSlotShape::conservative(list_java_type(JavaType::String));
    let optional_shape = RustSlotShape::conservative(optional_java_type(JavaType::String));

    assert_eq!(list_shape.sequence_element_type(), Some(JavaType::String));
    assert!(list_shape.is_sequence_of(&string_shape));
    assert_eq!(
        optional_shape.optional_element_type(),
        Some(JavaType::String)
    );
    assert!(optional_shape.is_optional_of(&string_shape));
}

#[test]
fn slot_shape_owns_conservative_java_to_rust_projection() {
    assert_eq!(
        RustSlotShape::conservative(list_java_type(JavaType::String)).rust_type(),
        RustType::Vec(Box::new(RustType::String))
    );

    assert_eq!(
        RustSlotShape::conservative(optional_java_type(JavaType::Primitive(PrimitiveType::Int)))
            .rust_type(),
        RustType::Option(Box::new(RustType::I32))
    );
}

#[test]
fn slot_shape_owns_default_expressions_for_planned_views() {
    let mut string_view = RustSlotShape::conservative(JavaType::String);
    string_view.ownership = OwnershipShape::SharedBorrow;
    string_view.view = ViewShape::Str;
    assert_eq!(
        string_view.default_expr(),
        RustExpr::Literal("\"\"".to_string())
    );

    let mut slice_view = RustSlotShape::conservative(JavaType::Array(Box::new(JavaType::String)));
    slice_view.ownership = OwnershipShape::SharedBorrow;
    slice_view.view = ViewShape::Slice;
    assert_eq!(slice_view.default_expr(), RustExpr::Raw("&[]".to_string()));

    assert_eq!(
        RustSlotShape::conservative(JavaType::String).default_expr(),
        RustExpr::Raw("String::new()".to_string())
    );
}

#[test]
fn representation_plan_keeps_interface_dispatch_as_fallback_policy() {
    let parsed = parse_java_source(
        r#"
interface Animal {
    String name();
}

class Dog implements Animal {
    public String name() { return "dog"; }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);

    assert_eq!(
        representations.representation_for(&JavaType::Named {
            name: "Animal".to_string(),
            args: Vec::new()
        }),
        ObjectRepresentation::DynTrait {
            trait_name: "Animal".to_string()
        }
    );
    assert_eq!(
        representations.dispatch_for(&JavaType::Named {
            name: "Animal".to_string(),
            args: Vec::new()
        }),
        DispatchShape::DynTrait {
            trait_name: "Animal".to_string()
        }
    );
}

#[test]
fn representation_plan_records_hierarchy_edges_before_lowering() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Dog implements Animal {}
class Poodle extends Dog {}
class GhostDog implements Missing {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);

    assert_eq!(
        representations.direct_subtypes_of("Animal"),
        ["Dog".to_string()].as_slice()
    );
    assert_eq!(
        representations.direct_subtypes_of("Dog"),
        ["Poodle".to_string()].as_slice()
    );
}

#[test]
fn representation_plan_owns_object_model_unsupported_diagnostics() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Dog implements Animal {}
class Poodle extends Dog {}
class GhostDog implements Missing {}
"#,
    );
    assert!(!parsed.diagnostics.has_code("unsupported_interface"));
    assert!(!parsed.diagnostics.has_code("unsupported_interface_impl"));
    assert!(!parsed.diagnostics.has_code("unsupported_inheritance"));

    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let diagnostics = representations.diagnostics_for_program(&resolved.program);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code != "unsupported_interface")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported_interface_impl")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported_inheritance")
    );
}

#[test]
fn representation_plan_owns_anonymous_class_diagnostics() {
    let parsed = parse_java_source(
        r#"
class Factory {
    interface Value {
        int get();
    }

    Value make() {
        return new Value() {
            public int get() { return 1; }
        };
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let diagnostics = representations.diagnostics_for_program(&resolved.program);

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unsupported_anonymous_class")
    );
}

#[test]
fn representation_plan_applies_user_closed_enum_hierarchy_override() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add implements Expr {}
class Num implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = FactSet::default();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Expr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string(), "Num".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });

    let representations = plan_representations(&resolved.program, &facts);

    assert_eq!(
        representations.representation_for(&JavaType::Named {
            name: "Expr".to_string(),
            args: Vec::new(),
        }),
        ObjectRepresentation::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string(), "Num".to_string()],
        }
    );
    assert_eq!(
        representations.dispatch_for(&JavaType::Named {
            name: "Expr".to_string(),
            args: Vec::new(),
        }),
        DispatchShape::ClosedEnum {
            enum_name: "ExprValue".to_string(),
        }
    );
    assert_eq!(
        representations.direct_subtypes_of("Expr"),
        ["Add".to_string(), "Num".to_string()].as_slice()
    );
    assert_eq!(
        representations
            .type_representation("Expr")
            .unwrap()
            .provenance,
        RepresentationProvenance::UserOverride
    );
    assert!(
        representations
            .diagnostics_for_program(&resolved.program)
            .iter()
            .all(|diagnostic| diagnostic.code != "representation_override_conflict")
    );
}

#[test]
fn representation_plan_owns_non_struct_type_emission() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add implements Expr {}
class Num implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = FactSet::default();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Expr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string(), "Num".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });

    let representations = plan_representations(&resolved.program, &facts);

    assert_eq!(
        representations.type_emission_for("Expr"),
        RepresentationTypeEmission::RawRustItem(
            "pub enum ExprValue {\n    Add(Add),\n    Num(Num),\n}".to_string()
        )
    );
    assert_eq!(
        representations.type_emission_for("Add"),
        RepresentationTypeEmission::LowerAsConcreteStruct
    );
}

#[test]
fn representation_plan_reports_closed_enum_override_conflicts() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add implements Expr {}
class Num implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = FactSet::default();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Expr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string(), "Missing".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });

    let representations = plan_representations(&resolved.program, &facts);
    let diagnostics = representations.diagnostics_for_program(&resolved.program);

    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.code == "representation_override_conflict"
                && diagnostic.message.contains("unknown variant Missing")
        ),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.code == "representation_override_conflict"
                && diagnostic.message.contains("omits source subtype Num")
        ),
        "{diagnostics:?}"
    );
}

#[test]
fn signature_plan_owns_local_shapes_before_lowering() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add implements Expr {}
class Use {
    boolean check(Add add) {
        Expr expr = (Expr) add;
        return expr instanceof Add;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Expr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let use_ty = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Use")
        .unwrap();
    let method = use_ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "check" => Some(method),
            _ => None,
        })
        .unwrap();
    let method_signature = signatures.method_signature(use_ty, method).unwrap();
    let expr_shape = signatures
        .local_shape(&method_signature.locals, "expr")
        .expect("expr local should have a solved shape");

    assert_eq!(
        expr_shape.java_ty,
        JavaType::Named {
            name: "Expr".to_string(),
            args: Vec::new(),
        }
    );
    assert_eq!(
        expr_shape.dispatch,
        DispatchShape::ClosedEnum {
            enum_name: "ExprValue".to_string()
        }
    );
    assert!(
        signatures
            .local_shape(&method_signature.locals, "missing")
            .is_none()
    );

    let lowered = lower_program_with_plans(
        &resolved.program,
        resolved.diagnostics,
        &facts,
        &usage,
        &representations,
        &signatures,
    );
    let rust = emit::emit_file(&lowered.file);

    assert!(
        rust.contains("let expr: ExprValue = ExprValue::Add(add);"),
        "{rust}"
    );
    assert!(
        rust.contains("return matches!(expr, ExprValue::Add(_));"),
        "{rust}"
    );
}

#[test]
fn signature_plan_owns_local_binding_mutability_before_lowering() {
    let parsed = parse_java_source(
        r#"
class Use {
    int calc() {
        int stable = 1;
        int changed = stable;
        changed = changed + 1;
        return stable + changed;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "calc" => Some(method),
            _ => None,
        })
        .unwrap();
    let method_signature = signatures.method_signature(ty, method).unwrap();
    let local_mutability = |name: &str| {
        signatures
            .local_binding_mutability(&method_signature.locals, name)
            .unwrap()
    };

    assert!(!local_mutability("stable"));
    assert!(local_mutability("changed"));
    assert!(
        signatures
            .local_binding_mutability(&method_signature.locals, "missing")
            .is_none()
    );

    let lowered = lower_program_with_plans(
        &resolved.program,
        resolved.diagnostics,
        &FactSet::default(),
        &usage,
        &representations,
        &signatures,
    );
    let rust = emit::emit_file(&lowered.file);

    assert!(rust.contains("let stable: i32 = 1;"), "{rust}");
    assert!(rust.contains("let mut changed: i32 = stable;"), "{rust}");
}

#[test]
fn constructor_field_initializers_use_planned_field_shapes() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add implements Expr {
    Add() {}
}
class Holder {
    Expr expr = (Expr) new Add();
    Holder() {}
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Expr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);

    assert_eq!(
        signatures.field_shape("Holder", "expr").unwrap().dispatch,
        DispatchShape::ClosedEnum {
            enum_name: "ExprValue".to_string()
        }
    );

    let lowered = lower_program_with_plans(
        &resolved.program,
        resolved.diagnostics,
        &facts,
        &usage,
        &representations,
        &signatures,
    );
    let rust = emit::emit_file(&lowered.file);

    assert!(rust.contains("expr: ExprValue"), "{rust}");
    assert!(rust.contains("expr: ExprValue::Add(Add::new())"), "{rust}");
}

#[test]
fn representation_plan_owns_constructor_owner_type_for_new_expressions() {
    let parsed = parse_java_source(
        r#"
class Add {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);

    assert_eq!(
        representations.constructor_owner_type_for_new(
            &JavaType::Unknown("anonymous".to_string()),
            Some("Add"),
        ),
        RustType::Named("Add".to_string())
    );
}

#[test]
fn throwing_method_result_ok_type_uses_planned_return_shape() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add implements Expr {
    Add() {}
}
class Maker {
    Expr make() throws Exception {
        return (Expr) new Add();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Expr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let maker = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Maker")
        .unwrap();
    let method = maker
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "make" => Some(method),
            _ => None,
        })
        .unwrap();

    assert_eq!(
        signatures
            .method_return_shape(maker, method)
            .unwrap()
            .rust_type(),
        RustType::Named("ExprValue".to_string())
    );

    let lowered = lower_program_with_plans(
        &resolved.program,
        resolved.diagnostics,
        &facts,
        &usage,
        &representations,
        &signatures,
    );
    let rust = emit::emit_file(&lowered.file);

    assert!(
        rust.contains("pub fn make(&self) -> Result<ExprValue, JavaException>"),
        "{rust}"
    );
    assert!(
        rust.contains("return Ok(ExprValue::Add(Add::new()));"),
        "{rust}"
    );
}

#[test]
fn expression_shape_context_uses_representation_for_cast_dispatch() {
    let parsed = parse_java_source(
        r#"
interface Expr {}
class Add implements Expr {}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Expr".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "ExprValue".to_string(),
            variants: vec!["Add".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });
    let representations = plan_representations(&resolved.program, &facts);
    let mut env = TypeEnv::new();
    env.insert_local(
        "add",
        JavaType::Named {
            name: "Add".to_string(),
            args: Vec::new(),
        },
    );

    let shape = infer_expr_shape_with_context(
        &parse_expr("(Expr) add"),
        &env,
        ExprShapeContext {
            signatures: None,
            representations: Some(&representations),
        },
    );

    assert_eq!(
        shape.dispatch,
        DispatchShape::ClosedEnum {
            enum_name: "ExprValue".to_string()
        }
    );
    assert_eq!(shape.rust_type(), RustType::Named("ExprValue".to_string()));
}

#[test]
fn cast_lowering_uses_representation_shaped_coercion_for_closed_enums() {
    let parsed = parse_java_source(
        r#"
interface Animal {}
class Dog implements Animal {}
class Use {
    Animal up(Dog dog) {
        return (Animal) dog;
    }

    Dog down(Animal animal) {
        return (Dog) animal;
    }

    boolean isDog(Animal animal) {
        return animal instanceof Dog;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let mut facts = resolved.facts.clone();
    facts.hierarchy_overrides.push(HierarchyOverride {
        type_name: "Animal".to_string(),
        strategy: HierarchyStrategy::ClosedEnum {
            enum_name: "AnimalValue".to_string(),
            variants: vec!["Dog".to_string()],
        },
        provenance: FactProvenance::UserOverride,
    });
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let lowered = lower_program_with_plans(
        &resolved.program,
        resolved.diagnostics,
        &facts,
        &usage,
        &representations,
        &signatures,
    );
    let rust = emit::emit_file(&lowered.file);

    assert!(rust.contains("pub enum AnimalValue"), "{rust}");
    assert!(
        rust.contains("pub fn up(&self, dog: Dog) -> AnimalValue"),
        "{rust}"
    );
    assert!(rust.contains("return AnimalValue::Dog(dog);"), "{rust}");
    assert!(
        rust.contains("pub fn down(&self, animal: AnimalValue) -> Dog"),
        "{rust}"
    );
    assert!(
        rust.contains(
            "return match animal { AnimalValue::Dog(value) => value, _ => panic!(\"invalid Java cast\") };"
        ),
        "{rust}"
    );
    assert!(
        rust.contains("pub fn is_dog(&self, animal: AnimalValue) -> bool"),
        "{rust}"
    );
    assert!(
        rust.contains("return matches!(animal, AnimalValue::Dog(_));"),
        "{rust}"
    );
}

#[test]
fn signature_plan_owns_field_param_and_return_shapes_before_lowering() {
    let parsed = parse_java_source(
        r#"
class Holder {
    String name;
    Holder(String name) { this.name = name; }
    public String echo(String value) { return value; }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let usage = analyze_usage(&resolved.program);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();
    let ctor = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Constructor(ctor) => Some(ctor),
            _ => None,
        })
        .unwrap();

    let field_shape = signatures.field_shape("Holder", "name").unwrap();
    assert_eq!(field_shape.java_ty, JavaType::String);
    assert_eq!(field_shape.mutability, MutabilityShape::Mutable);
    assert!(signatures.field_shape("Holder", "missing").is_none());

    let method_signature = signatures.method_signature(ty, method).unwrap();
    assert_eq!(method_signature.params[0].java_ty, JavaType::String);
    assert_eq!(
        signatures.method_param_plans(ty, method).unwrap()[0].java_ty,
        JavaType::String
    );
    assert_eq!(
        signatures.constructor_param_plans(ty, ctor).unwrap()[0].java_ty,
        JavaType::String
    );
    assert_eq!(
        signatures.constructor_return_type(ty, ctor).unwrap(),
        RustType::Named("Holder".to_string())
    );
    assert_eq!(
        method_signature.return_shape.as_ref().unwrap().java_ty,
        JavaType::String
    );
}

#[test]
fn signature_plan_owns_receiver_mutability_before_lowering() {
    let parsed = parse_java_source(
        r#"
class Counter {
    int value;

    public int get() {
        return this.value;
    }

    public void set(int next) {
        this.value = next;
    }

    public static int id(int value) {
        return value;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = &resolved.program.types[0];

    let receiver_for = |name: &str| {
        let method = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == name => Some(method),
                _ => None,
            })
            .unwrap();
        signatures.method_receiver(ty, method).unwrap()
    };

    assert_eq!(receiver_for("get"), ReceiverPlan::Shared);
    assert_eq!(receiver_for("set"), ReceiverPlan::Mutable);
    assert_eq!(receiver_for("id"), ReceiverPlan::None);

    let get_method = ty
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "get" => Some(method),
            _ => None,
        })
        .unwrap();
    let mut wrong_type = ty.clone();
    wrong_type.name = "MissingCounter".to_string();
    assert!(
        signatures
            .method_receiver(&wrong_type, get_method)
            .is_none()
    );
}

#[test]
fn signature_solver_can_choose_borrowed_str_for_private_content_only_params() {
    let parsed = parse_java_source(
        r#"
class StringUse {
    private int len(String value) {
        return value.length();
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let ty = &resolved.program.types[0];
    let method = ty
        .members
        .iter()
        .find_map(|member| match member {
            new_java2rust::java_ir::Member::Method(method) => Some(method),
            _ => None,
        })
        .unwrap();

    let param = &signatures.method_param_plans(ty, method).unwrap()[0];
    assert_eq!(param.shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(param.shape.view, ViewShape::Str);
    assert_eq!(
        param.shape.rust_type(),
        RustType::Ref {
            mutable: false,
            inner: Box::new(RustType::Raw("str".to_string()))
        }
    );
}

#[test]
fn call_lowering_uses_callee_param_shape_for_argument_coercion() {
    let translation = translate_source(
        r#"
class StringCalls {
    private int emptyScore(String value) {
        return value.isEmpty() ? 1 : 0;
    }

    public int call(String value) {
        return emptyScore(value);
    }
}
"#,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("pub fn empty_score(&self, value: &str) -> i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("self.empty_score(value.as_str())"),
        "{}",
        translation.rust
    );
}

#[test]
fn cross_class_call_lowering_uses_declaring_type_param_shape() {
    let translation = translate_source(
        r#"
class Helper {
    int score(String value) {
        return value.isEmpty() ? 1 : 0;
    }
}

class Caller {
    public int call(String value) {
        Helper helper = new Helper();
        return helper.score(value);
    }
}
"#,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("pub fn score(&self, value: &str) -> i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("helper.score(value.as_str())"),
        "{}",
        translation.rust
    );
}

#[test]
fn new_expression_uses_constructor_param_shape_for_argument_coercion() {
    let translation = translate_source(
        r#"
class StringBox {
    int size;
    StringBox(String value) {
        this.size = value.length();
    }
}

class MakesBox {
    public StringBox make(String value) {
        return new StringBox(value);
    }
}
"#,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("pub fn new(value: &str) -> StringBox"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("StringBox::new(value.as_str())"),
        "{}",
        translation.rust
    );
}

#[test]
fn shape_env_prevents_redundant_string_view_adapters() {
    let translation = translate_source(
        r#"
class StringFlow {
    private int inner(String value) {
        return value.length();
    }

    private int outer(String value) {
        return inner(value);
    }
}
"#,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation
            .rust
            .contains("pub fn inner(&self, value: &str) -> i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("pub fn outer(&self, value: &str) -> i32"),
        "{}",
        translation.rust
    );
    assert!(
        translation.rust.contains("self.inner(value)"),
        "{}",
        translation.rust
    );
    assert!(
        !translation.rust.contains("self.inner(value.as_str())"),
        "{}",
        translation.rust
    );
}

#[test]
fn call_return_shape_drives_return_slot_coercion() {
    let translation = translate_source(
        r#"
class StringReturnFlow {
    private static String id(String value) {
        return value;
    }

    public String call(String value) {
        return id(value);
    }
}
"#,
    );

    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(
        translation.rust.contains("pub fn id(value: &str) -> &str"),
        "{}",
        translation.rust
    );
    assert!(
        translation
            .rust
            .contains("return Self::id(value.as_str()).to_string();"),
        "{}",
        translation.rust
    );
}

#[test]
fn usage_analysis_records_body_facts_without_emitting_rust_shapes() {
    let parsed = parse_java_source(
        r#"
class Uses {
    String name;
    public int len(String value) {
        this.name = value;
        return value.length();
    }
}
"#,
    );
    let usage = analyze_usage(&parsed.program);
    let method = usage
        .methods
        .values()
        .find(|method| method.bindings.contains_key("value"))
        .unwrap();
    let value = method.bindings.get("value").unwrap();

    assert!(
        value
            .uses
            .iter()
            .any(|usage| matches!(usage, BindingUse::Stored))
    );
    assert!(
        value
            .uses
            .iter()
            .any(|usage| matches!(usage, BindingUse::ContentRead))
    );
}

#[test]
fn type_env_records_declared_locals_as_storage_shapes() {
    let mut env = TypeEnv::new();
    env.learn_stmt(&Stmt::VarDecl {
        ty: JavaType::String,
        name: "name".to_string(),
        initializer: None,
    });

    let shape = env.local_shape("name").unwrap();
    assert_eq!(shape.java_ty, JavaType::String);
    assert_eq!(shape.ownership, OwnershipShape::Owned);
    assert_eq!(shape.view, ViewShape::Native);
    assert_eq!(shape.mutability, MutabilityShape::Mutable);
}

#[test]
fn indexed_shape_projects_borrowed_slice_elements_without_lowering_rules() {
    let mut env = TypeEnv::new();
    let mut values_shape = RustSlotShape::conservative(JavaType::Array(Box::new(JavaType::String)));
    values_shape.ownership = OwnershipShape::SharedBorrow;
    values_shape.view = ViewShape::Slice;
    env.insert_local_shape("values", values_shape);

    let element_shape = infer_expr_shape(&parse_expr("values[0]"), &env);

    assert_eq!(element_shape.java_ty, JavaType::String);
    assert_eq!(element_shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(element_shape.view, ViewShape::Native);
}

#[test]
fn indexed_assignment_target_shape_projects_storage_element_shape() {
    let mut env = TypeEnv::new();
    let mut values_shape = RustSlotShape::conservative(JavaType::Array(Box::new(JavaType::String)));
    values_shape.ownership = OwnershipShape::SharedBorrow;
    values_shape.view = ViewShape::Slice;
    env.insert_local_shape("values", values_shape);

    let read_shape = infer_expr_shape(&parse_expr("values[0]"), &env);
    let target_shape = infer_assignment_target_shape(&parse_expr("values[0]"), &env).unwrap();

    assert_eq!(read_shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(target_shape.java_ty, JavaType::String);
    assert_eq!(target_shape.ownership, OwnershipShape::Owned);
    assert_eq!(target_shape.view, ViewShape::Native);
    assert_eq!(target_shape.mutability, MutabilityShape::Mutable);
}

#[test]
fn indexed_shape_keeps_copy_elements_as_values() {
    let mut env = TypeEnv::new();
    let mut values_shape = RustSlotShape::conservative(JavaType::Array(Box::new(
        JavaType::Primitive(PrimitiveType::Int),
    )));
    values_shape.ownership = OwnershipShape::SharedBorrow;
    values_shape.view = ViewShape::Slice;
    env.insert_local_shape("values", values_shape);

    let element_shape = infer_expr_shape(&parse_expr("values[0]"), &env);

    assert_eq!(
        element_shape.java_ty,
        JavaType::Primitive(PrimitiveType::Int)
    );
    assert_eq!(element_shape.ownership, OwnershipShape::CopyValue);
}

#[test]
fn cast_shape_is_owned_by_type_inference_not_lowering() {
    let mut env = TypeEnv::new();
    env.insert_local("value", JavaType::Unknown("Object".to_string()));

    let shape = infer_expr_shape(&parse_expr("(String) value"), &env);

    assert_eq!(shape.java_ty, JavaType::String);
    assert_eq!(shape.ownership, OwnershipShape::Owned);
    assert_eq!(shape.view, ViewShape::Native);
}

#[test]
fn stream_lambda_body_shape_owns_borrowed_string_views() {
    let default = RustSlotShape::conservative(JavaType::String);
    let shape = infer_stream_lambda_body_shape(&parse_expr("value.trim()"), &default);

    assert_eq!(shape.java_ty, JavaType::String);
    assert_eq!(shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(shape.view, ViewShape::Str);
}

#[test]
fn stream_source_element_shape_is_inferred_before_lowering() {
    let mut env = TypeEnv::new();
    env.insert_local("values", list_java_type(JavaType::String));

    let shape = infer_stream_source_element_shape(&parse_expr("values"), &env).unwrap();

    assert_eq!(shape.java_ty, JavaType::String);
    assert_eq!(shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(shape.view, ViewShape::Native);
}

#[test]
fn stream_map_to_int_shape_is_inferred_before_lowering() {
    let shape = infer_stream_map_to_int_shape(&parse_expr("value.length()"));

    assert_eq!(shape.java_ty, JavaType::Primitive(PrimitiveType::Int));
    assert_eq!(shape.ownership, OwnershipShape::CopyValue);
    assert_eq!(shape.view, ViewShape::Native);
}

#[test]
fn stream_collect_lowered_type_is_shape_driven() {
    let mut element = RustSlotShape::conservative(JavaType::String);
    element.ownership = OwnershipShape::SharedBorrow;
    element.view = ViewShape::Str;

    let lowered = LoweredType::stream_collect(element);

    assert!(matches!(
        lowered,
        LoweredType::VecElementBorrow { element_shape, .. }
            if element_shape.java_ty == JavaType::String
                && element_shape.ownership == OwnershipShape::SharedBorrow
                && element_shape.view == ViewShape::Str
    ));
}

#[test]
fn resolved_call_return_shape_is_available_before_lowering() {
    let parsed = parse_java_source(
        r#"
class StringReturnFlow {
    private static String id(String value) {
        return value;
    }

    public String call(String value) {
        return id(value);
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let call_method = resolved.program.types[0]
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "call" => Some(method),
            _ => None,
        })
        .unwrap();
    let return_expr = call_method
        .body
        .as_ref()
        .unwrap()
        .statements
        .iter()
        .find_map(|stmt| match stmt {
            Stmt::Return(Some(expr)) => Some(expr),
            _ => None,
        })
        .unwrap();
    assert!(matches!(
        return_expr,
        Expr::Call {
            resolved: Some(_),
            ..
        }
    ));
    let solved_id_return = signatures
        .method_return_shape_by_rust_name("StringReturnFlow", "id")
        .expect("id should have a solved return shape");
    assert_eq!(solved_id_return.ownership, OwnershipShape::SharedBorrow);
    assert!(
        signatures
            .method_return_shape_by_rust_name("StringReturnFlow", "missing")
            .is_none()
    );

    let shape = infer_expr_shape_with_context(
        return_expr,
        &TypeEnv::new(),
        ExprShapeContext {
            signatures: Some(&signatures),
            representations: Some(&representations),
        },
    );

    assert_eq!(shape.java_ty, JavaType::String);
    assert_eq!(shape.ownership, OwnershipShape::SharedBorrow);
    assert_eq!(shape.view, ViewShape::Str);
}

#[test]
fn typed_receiver_field_shape_is_available_before_lowering() {
    let parsed = parse_java_source(
        r#"
class Helper {
    String name;
}

class Caller {
    public String read(Helper helper) {
        return helper.name;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);
    let caller = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Caller")
        .unwrap();
    let read_method = caller
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "read" => Some(method),
            _ => None,
        })
        .unwrap();
    let return_expr = read_method
        .body
        .as_ref()
        .unwrap()
        .statements
        .iter()
        .find_map(|stmt| match stmt {
            Stmt::Return(Some(expr)) => Some(expr),
            _ => None,
        })
        .unwrap();
    assert!(matches!(return_expr, Expr::FieldAccess(_, field) if field == "name"));

    let mut env = TypeEnv::new();
    env.insert_local(
        "helper",
        JavaType::Named {
            name: "Helper".to_string(),
            args: Vec::new(),
        },
    );
    let shape = infer_expr_shape_with_context(
        return_expr,
        &env,
        ExprShapeContext {
            signatures: Some(&signatures),
            representations: Some(&representations),
        },
    );

    assert_eq!(shape.java_ty, JavaType::String);
    assert_eq!(shape.ownership, OwnershipShape::Owned);
    assert_eq!(shape.view, ViewShape::Native);
    assert_eq!(shape.mutability, MutabilityShape::Mutable);
}

#[test]
fn typed_receiver_field_assignment_shape_is_available_before_lowering() {
    let parsed = parse_java_source(
        r#"
class Helper {
    String name;
}

class Caller {
    public void write(Helper helper, String value) {
        helper.name = value;
    }
}
"#,
    );
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    let usage = analyze_usage(&resolved.program);
    let representations = plan_representations(&resolved.program, &resolved.facts);
    let signatures = solve_signatures(&resolved.program, &representations, &usage);

    let mut env = TypeEnv::new();
    env.insert_local(
        "helper",
        JavaType::Named {
            name: "Helper".to_string(),
            args: Vec::new(),
        },
    );
    let shape = infer_assignment_target_shape_with_context(
        &parse_expr("helper.name"),
        &env,
        ExprShapeContext {
            signatures: Some(&signatures),
            representations: Some(&representations),
        },
    )
    .unwrap();

    assert_eq!(shape.java_ty, JavaType::String);
    assert_eq!(shape.ownership, OwnershipShape::Owned);
    assert_eq!(shape.view, ViewShape::Native);
    assert_eq!(shape.mutability, MutabilityShape::Mutable);
}
