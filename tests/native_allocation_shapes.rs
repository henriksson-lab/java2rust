#[path = "common/mod.rs"]
mod common;

#[test]
fn indexed_collection_elements_preserve_root_and_nested_allocation_qualifiers() {
    use new_java2rust::{
        facts::FactSet,
        java_ir::{JavaType, PrimitiveType},
        slot_shape::RustSlotShape,
        storage_strategy::StoragePlacement,
    };
    let inner = JavaType::Named {
        name: "List".into(),
        args: vec![JavaType::Primitive(PrimitiveType::Int)],
    };
    let element = JavaType::Named {
        name: "List".into(),
        args: vec![inner],
    };
    for nullable in [false, true] {
        for sync in [false, true] {
            let mut array = RustSlotShape::for_storage(JavaType::Array(Box::new(element.clone())));
            if nullable {
                array.java_ty = new_java2rust::types::optional_java_type(array.java_ty);
            }
            let shared = StoragePlacement::new(true, true, sync);
            array.nested_native_allocations = vec![(vec![0], shared), (vec![0, 0], shared)];
            let read = new_java2rust::shape_projection::indexed_read_shape_for_access_with_facts(
                &array,
                &FactSet::default(),
            )
            .unwrap();
            let write = new_java2rust::shape_projection::indexed_assignment_shape_with_facts(
                &array,
                &FactSet::default(),
            )
            .unwrap();
            for child in [read, write] {
                assert_eq!(child.native_allocation, shared);
                assert_eq!(child.nested_native_allocations, vec![(vec![0], shared)]);
            }
        }
    }
}

#[test]
fn retained_iteration_null_assignment_keeps_nullable_storage() {
    use new_java2rust::{
        facts::FactSet, java_ir::Member, parser, representation, resolve, semantic_analysis,
        signature,
    };
    let parsed = parser::parse_java_source(include_str!("fixtures/retained_native_iteration.java"));
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
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
    let method = owner
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "nullSource" => Some(method),
            _ => None,
        })
        .unwrap();
    let local = plan
        .required_method_signature(owner, method)
        .locals()
        .iter()
        .find(|local| local.name() == "values")
        .unwrap();
    assert!(local.shape().optional_element_type().is_some(), "{local:?}");
    let generic = resolved
        .program
        .types
        .iter()
        .find(|owner| owner.name == "GenericIteration")
        .unwrap();
    let method = generic
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "countNonNull" => Some(method),
            _ => None,
        })
        .unwrap();
    let binding = plan
        .required_method_signature(generic, method)
        .locals()
        .iter()
        .find(|local| local.name() == "value")
        .unwrap();
    assert!(
        binding.shape().optional_element_type().is_some(),
        "{binding:?}"
    );
    let declaration = generic
        .members
        .iter()
        .find_map(|member| match member {
            Member::Field(field) if field.name == "values" => Some(field),
            _ => None,
        })
        .unwrap();
    let field = plan.required_declared_field_shape(generic, declaration);
    let element = new_java2rust::types::sequence_element_type(&field.java_ty).unwrap();
    assert!(
        new_java2rust::types::optional_element_type(&element).is_none(),
        "{field:?}"
    );
}

#[test]
fn factory_retention_contract_uses_selected_jdk_owner_not_source_or_local_shadow() {
    use new_java2rust::{java_ir::Member, parser, resolve, usage};
    for (source, retained) in [
        (
            "class Probe { static <T> java.util.Set<T> run(T value) { return java.util.Collections.singleton(value); } }",
            true,
        ),
        (
            "class Collections { static <T> T singleton(T value) { return value; } } class Probe { static <T> T run(T value) { return Collections.singleton(value); } }",
            false,
        ),
        (
            "class Factory { <T> T singleton(T value) { return value; } } class Probe { static <T> T run(T value) { Factory Collections = new Factory(); return Collections.singleton(value); } }",
            false,
        ),
    ] {
        let parsed = parser::parse_java_source(source);
        let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let usage = usage::analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let owner = resolved
            .program
            .types
            .iter()
            .find(|owner| owner.name == "Probe")
            .unwrap();
        let method = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "run" => Some(method),
                _ => None,
            })
            .unwrap();
        let binding = usage
            .source_method_usage(owner, method)
            .unwrap()
            .all_binding_facts()
            .into_iter()
            .find(|binding| binding.binding() == method.params[0].binding.as_ref())
            .unwrap();
        assert_eq!(
            binding
                .uses()
                .iter()
                .any(|use_| matches!(use_, usage::BindingUse::RetainedByCall)),
            retained,
            "{source}"
        );
    }
}

#[test]
fn retained_factory_nested_iterable_layout_reaches_enhanced_for_binding() {
    use new_java2rust::{java_ir::Member, parser, representation, resolve, signature, usage};
    let parsed =
        parser::parse_java_source(include_str!("fixtures/retained_immutable_factories.java"));
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let usage = usage::analyze_usage_with_facts(&resolved.program, &resolved.facts);
    let representations = representation::plan_representations(&resolved.program, &resolved.facts);
    let plan = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &usage,
        &resolved.facts,
    );
    let owner = &resolved.program.types[0];
    let method = owner
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "nested" => Some(method),
            _ => None,
        })
        .unwrap();
    let signature = plan.required_method_signature(owner, method);
    let binding = signature
        .locals()
        .iter()
        .find(|local| local.name() == "from_view")
        .unwrap();
    assert!(binding.shape().native_allocation.shared(), "{:?}", binding);
}

#[test]
fn override_storage_join_preserves_nested_paths_without_retaining_the_outer_map() {
    use new_java2rust::{java_ir::Member, parser, representation, resolve, signature, usage};
    let parsed = parser::parse_java_source(
        r#"
import java.util.*;
interface NestedRows { Map<String,List<Integer>> rows(); }
class StoredRows implements NestedRows {
    Map<String,List<Integer>> data = new HashMap<>();
    public Map<String,List<Integer>> rows() {
        List<Integer> row = new ArrayList<>();
        data.put("x", row);
        return data;
    }
}
class EmptyRows implements NestedRows {
    public Map<String,List<Integer>> rows() { return new HashMap<>(); }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let usage = usage::analyze_usage_with_facts(&resolved.program, &resolved.facts);
    let representations = representation::plan_representations(&resolved.program, &resolved.facts);
    let plan = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &usage,
        &resolved.facts,
    );
    let mut checked = 0;
    for owner in &resolved.program.types {
        for member in &owner.members {
            if let Member::Method(method) = member {
                let shape = plan
                    .required_method_signature(owner, method)
                    .return_shape()
                    .unwrap();
                assert!(!shape.native_allocation.shared(), "{} outer", owner.name);
                assert_eq!(
                    shape.nested_native_allocations.len(),
                    1,
                    "{} paths: {:?}",
                    owner.name,
                    shape
                );
                assert_eq!(shape.nested_native_allocations[0].0, vec![1]);
                assert!(shape.nested_native_allocations[0].1.shared());
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 3);
}

#[test]
fn override_native_qualifiers_do_not_wrap_erased_type_parameters_or_unrelated_implementations() {
    use new_java2rust::{java_ir::Member, parser, representation, resolve, signature, usage};
    let parsed = parser::parse_java_source(
        r#"
import java.util.*;
interface GenericSlot<T> { T get(); }
class NativeGenericSlot implements GenericSlot<Set<Integer>> {
    List<Set<Integer>> rows = new ArrayList<>();
    public Set<Integer> get() { return rows.get(0); }
}
class PlainGenericSlot implements GenericSlot<String> {
    public String get() { return "plain"; }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let usage = usage::analyze_usage_with_facts(&resolved.program, &resolved.facts);
    let representations = representation::plan_representations(&resolved.program, &resolved.facts);
    let plan = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &usage,
        &resolved.facts,
    );
    let mut checked = 0;
    for owner in &resolved.program.types {
        for member in &owner.members {
            if let Member::Method(method) = member {
                let shape = plan
                    .required_method_signature(owner, method)
                    .return_shape()
                    .unwrap();
                assert_eq!(
                    shape.native_allocation.shared(),
                    owner.name == "NativeGenericSlot",
                    "{}",
                    owner.name
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 3);
}

#[test]
fn native_allocation_contract_is_shared_by_every_source_override() {
    use new_java2rust::{java_ir::Member, parser, representation, resolve, signature, usage};
    let parsed = parser::parse_java_source(include_str!("fixtures/retained_override_storage.java"));
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let usage = usage::analyze_usage_with_facts(&resolved.program, &resolved.facts);
    let representations = representation::plan_representations(&resolved.program, &resolved.facts);
    let plan = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &usage,
        &resolved.facts,
    );
    let mut checked = 0;
    for owner in &resolved.program.types {
        for member in &owner.members {
            let Member::Method(method) = member else {
                continue;
            };
            let signature = plan.required_method_signature(owner, method);
            match method.name.as_str() {
                "values" => assert!(
                    signature.return_shape().unwrap().native_allocation.shared(),
                    "{} return",
                    owner.name
                ),
                "accept" => assert!(
                    signature.params()[0].shape().native_allocation.shared(),
                    "{} parameter",
                    owner.name
                ),
                _ => continue,
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 6);
}

#[test]
fn retained_iterable_origins_seed_fields_and_method_results_without_retaining_unrelated_fields() {
    use new_java2rust::{java_ir::Member, parser, representation, resolve, signature, usage};
    let parsed = parser::parse_java_source(
        r#"
import java.util.*;
class Origins {
    List<Integer> direct = new ArrayList<>();
    List<Integer> external = new ArrayList<>();
    List<Integer> untouched = new ArrayList<>();
    static List<Integer> fresh() { return new ArrayList<>(); }
    void run(Origins other) {
        Iterable<Integer> a = direct;
        Iterable<Integer> b = other.external;
        Iterable<Integer> c = fresh();
    }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    let usage = usage::analyze_usage_with_facts(&resolved.program, &resolved.facts);
    let representations = representation::plan_representations(&resolved.program, &resolved.facts);
    let plan = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &usage,
        &resolved.facts,
    );
    let owner = &resolved.program.types[0];
    for member in &owner.members {
        match member {
            Member::Field(field) => assert_eq!(
                plan.required_declared_field_shape(owner, field)
                    .native_allocation
                    .shared(),
                field.name != "untouched",
                "{}",
                field.name
            ),
            Member::Method(method) if method.name == "fresh" => assert!(
                plan.required_method_signature(owner, method)
                    .return_shape()
                    .unwrap()
                    .native_allocation
                    .shared()
            ),
            _ => {}
        }
    }
}

#[test]
fn nested_allocation_solver_preserves_lexical_identity_fields_and_call_boundaries() {
    use new_java2rust::{java_ir::Member, parser, representation, resolve, signature, usage};
    let parsed = parser::parse_java_source(
        r#"
import java.util.List;
import java.util.Map;
class Holder {
    Map<String, List<Integer>> data = new java.util.HashMap<>();
    Holder(List<Integer> input) { data.put("x", input); }
}
class ArrayList<T> { void add(T value) {} }
class Scope {
    static void run(Holder holder) {
        List<List<Integer>> rows = new java.util.ArrayList<>();
        { List<Integer> value = new java.util.ArrayList<>(); rows.add(value); }
        { List<Integer> value = new java.util.ArrayList<>(); rows.add(value); }
        { List<Integer> value = new java.util.ArrayList<>(); value.add(1); }
        List<Integer> alias = rows.get(0);
        holder.data.put("x", alias);
    }
    static List<Integer> first(List<List<Integer>> rows) { return rows.get(0); }
    static void append(List<List<Integer>> rows, List<Integer> value) { rows.add(value); }
    static void cycle() {
        List<Object> raw = new java.util.ArrayList<>();
        raw.add(raw);
    }
    static void sourceLookalike(ArrayList<List<Integer>> rows, List<Integer> value) { rows.add(value); }
}
"#,
    );
    let resolved = resolve::resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );
    let usage = usage::analyze_usage_with_facts(&resolved.program, &resolved.facts);
    let representations = representation::plan_representations(&resolved.program, &resolved.facts);
    let plan = signature::solve_signatures_with_facts(
        &resolved.program,
        &representations,
        &usage,
        &resolved.facts,
    );
    let scope = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Scope")
        .unwrap();
    for member in &scope.members {
        let Member::Method(method) = member else {
            continue;
        };
        let signature = plan.method_signature(scope, method).unwrap();
        match method.name.as_str() {
            "run" => {
                let values = signature
                    .locals()
                    .iter()
                    .filter(|local| local.binding().name() == "value")
                    .collect::<Vec<_>>();
                assert_eq!(values.len(), 3);
                assert_eq!(
                    values
                        .iter()
                        .filter(|value| value.shape().native_allocation.shared())
                        .count(),
                    2
                );
                assert_ne!(values[0].binding(), values[1].binding());
                let alias = signature
                    .locals()
                    .iter()
                    .find(|local| local.name() == "alias")
                    .unwrap();
                assert!(alias.shape().native_allocation.shared());
            }
            "first" => {
                assert!(signature.return_shape().unwrap().native_allocation.shared());
                assert!(
                    signature.params()[0]
                        .shape()
                        .nested_native_allocations
                        .iter()
                        .any(|(path, placement)| path == &[0] && placement.shared())
                );
            }
            "append" => {
                assert_eq!(
                    signature.params()[0].shape().view,
                    new_java2rust::slot_shape::ViewShape::Native
                );
            }
            "cycle" => assert!(
                signature
                    .locals()
                    .iter()
                    .all(|local| local.shape().nested_native_allocations.is_empty())
            ),
            "sourceLookalike" => assert!(!signature.params()[1].shape().native_allocation.shared()),
            _ => {}
        }
    }
    let holder = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Holder")
        .unwrap();
    let field = holder
        .members
        .iter()
        .find_map(|member| match member {
            Member::Field(field) => Some(field),
            _ => None,
        })
        .unwrap();
    let shape = plan.required_declared_field_shape(holder, field);
    assert!(
        shape
            .nested_native_allocations
            .iter()
            .any(|(path, placement)| path == &[1] && placement.shared())
    );
}

#[test]
fn nested_allocation_rendering_preserves_positions_and_outer_wrappers() {
    use new_java2rust::{
        emit,
        facts::FactSet,
        java_ir::{JavaType, PrimitiveType},
        slot_render,
        slot_shape::RustSlotShape,
        storage_strategy::StoragePlacement,
    };
    let named = |name: &str, args| JavaType::Named {
        name: name.into(),
        args,
    };
    let set = named(
        "java.util.Set",
        vec![JavaType::Primitive(PrimitiveType::Int)],
    );
    let shared = StoragePlacement::new(true, true, false);
    for nullable in [false, true] {
        for outer_shared in [false, true] {
            let ty = named("java.util.Map", vec![set.clone(), set.clone()]);
            let mut shape = RustSlotShape::for_storage(if nullable {
                new_java2rust::types::optional_java_type(ty)
            } else {
                ty
            });
            if outer_shared {
                shape.native_allocation = shared;
            }
            shape.nested_native_allocations = vec![(vec![1], shared)];
            let rendered = emit::emit_type(&slot_render::rust_type_for_slot_with_facts(
                &shape,
                &FactSet::default(),
            ));
            assert!(rendered.contains("HashMap<std::collections::HashSet<i32>, std::rc::Rc<std::cell::RefCell<JavaNativeCollection<i32>>>>"), "{rendered}");
            assert_eq!(
                rendered.matches("JavaNativeCollection").count(),
                1,
                "{rendered}"
            );
            assert_eq!(rendered.starts_with("Option<"), nullable, "{rendered}");
            assert_eq!(
                rendered.matches("std::rc::Rc<").count(),
                if outer_shared { 2 } else { 1 },
                "{rendered}"
            );
        }
    }
    let mut shape = RustSlotShape::for_storage(named(
        "java.util.Map",
        vec![JavaType::String, named("java.util.List", vec![set])],
    ));
    shape.nested_native_allocations = vec![(vec![1, 0], shared)];
    let rendered = emit::emit_type(&slot_render::rust_type_for_slot_with_facts(
        &shape,
        &FactSet::default(),
    ));
    assert!(
        rendered.contains("Vec<std::rc::Rc<std::cell::RefCell<JavaNativeCollection<i32>>>>"),
        "{rendered}"
    );
}

#[test]
fn retained_collection_carriers_are_not_materialized_as_value_collections() {
    use new_java2rust::{
        coercion, conversion_plan, emit,
        facts::FactSet,
        java_ir::JavaType,
        rust_ir::RustExpr,
        slot_shape::{OwnershipShape, RustSlotShape, ViewShape},
        storage_strategy::StoragePlacement,
    };
    let facts = FactSet::default();
    let mut actual = RustSlotShape::for_storage(JavaType::Named {
        name: "java.util.LinkedHashSet".into(),
        args: vec![JavaType::String],
    });
    let mut target = RustSlotShape::for_storage(JavaType::Named {
        name: "java.util.Set".into(),
        args: vec![JavaType::String],
    });
    target.view = ViewShape::JavaSemanticHash;
    // The value-backed case must still perform its existing carrier adapter.
    let value_plan = conversion_plan::shape_coercion_plan_with_facts(&actual, &target, &facts);
    let value = emit::emit_expr(&coercion::apply_shape_coercion_plan(
        RustExpr::Var("values".into()),
        &value_plan,
    ));
    assert!(
        value.contains("collect::<JavaSemanticHashSet<_>>"),
        "{value}"
    );
    for sync in [false, true] {
        actual.native_allocation = StoragePlacement::new(true, true, sync);
        target.native_allocation = actual.native_allocation;
        for ownership in [
            OwnershipShape::Owned,
            OwnershipShape::SharedBorrow,
            OwnershipShape::MutableBorrow,
        ] {
            actual.ownership = ownership;
            let plan = conversion_plan::shape_coercion_plan_with_facts(&actual, &target, &facts);
            assert!(
                plan.unsupported_diagnostics().is_empty(),
                "{:?}",
                plan.unsupported_diagnostics()
            );
            let handle = emit::emit_expr(&coercion::apply_shape_coercion_plan(
                RustExpr::Var("values".into()),
                &plan,
            ));
            if ownership == OwnershipShape::Owned {
                assert_eq!(handle, "values");
            } else {
                assert!(handle.contains("Clone::clone"), "{handle}");
            }
            assert!(
                !handle.contains("into_iter") && !handle.contains("Into::<"),
                "{handle}"
            );
        }
    }
}

#[test]
fn native_allocation_handle_recovery_preserves_identity_without_payload_clone() {
    use new_java2rust::{
        coercion, conversion_plan, emit,
        facts::FactSet,
        java_ir::JavaType,
        rust_ir::RustExpr,
        slot_render,
        slot_shape::{OwnershipShape, RustSlotShape},
        storage_strategy::StoragePlacement,
    };
    let facts = FactSet::default();
    let array = JavaType::Array(Box::new(JavaType::Named {
        name: "Opaque".to_string(),
        args: Vec::new(),
    }));
    let mut source = "struct Opaque(i32);\n".to_string();
    let mut assertions = String::new();
    for sync in [false, true] {
        let family = if sync { "sync" } else { "local" };
        let handle = if sync {
            "std::sync::Arc"
        } else {
            "std::rc::Rc"
        };
        let cell = if sync {
            "std::sync::Mutex"
        } else {
            "std::cell::RefCell"
        };
        let read = if sync { "lock().unwrap()" } else { "borrow()" };
        let write = if sync {
            "lock().unwrap()"
        } else {
            "borrow_mut()"
        };
        assertions.push_str(&format!(
            "let mut {family} = {handle}::new({cell}::new(vec![Some(Opaque(1))]));\n"
        ));
        for mutable in [false, true] {
            for nullable in [false, true] {
                let name = format!("recover_{family}_{mutable}_{nullable}");
                let mut target = RustSlotShape::for_storage(if nullable {
                    new_java2rust::types::optional_java_type(array.clone())
                } else {
                    array.clone()
                });
                target.native_allocation = StoragePlacement::new(true, true, sync);
                target.ownership = OwnershipShape::Owned;
                let mut actual = target.clone();
                actual.ownership = if mutable {
                    OwnershipShape::MutableBorrow
                } else {
                    OwnershipShape::SharedBorrow
                };
                actual.view = new_java2rust::slot_shape::ViewShape::Slice;
                let plan =
                    conversion_plan::shape_coercion_plan_with_facts(&actual, &target, &facts);
                assert!(
                    plan.unsupported_diagnostics().is_empty(),
                    "{:?}",
                    plan.unsupported_diagnostics()
                );
                let input =
                    emit::emit_type(&slot_render::rust_type_for_slot_with_facts(&actual, &facts));
                let output =
                    emit::emit_type(&slot_render::rust_type_for_slot_with_facts(&target, &facts));
                let value = emit::emit_expr(&coercion::apply_shape_coercion_plan(
                    RustExpr::Var("value".to_string()),
                    &plan,
                ));
                assert!(!value.contains("to_vec"), "{value}");
                assert!(!value.contains("TODO"), "{value}");
                source.push_str(&format!(
                    "fn {name}(value: {input}) -> {output} {{ {value} }}\n"
                ));
                let argument = format!("&{}{family}", if mutable { "mut " } else { "" });
                let argument = if nullable {
                    format!("Some({argument})")
                } else {
                    argument
                };
                assertions.push_str(&format!(
                    "let alias = {name}({argument}){}; assert!({handle}::ptr_eq(&alias, &{family})); alias.{write}[0].as_mut().unwrap().0 += 1; let observed = alias.{read}[0].as_ref().unwrap().0; let original = {family}.{read}[0].as_ref().unwrap().0; assert_eq!(observed, original);\n",
                    if nullable { ".unwrap()" } else { "" },
                ));
                if nullable {
                    assertions.push_str(&format!("assert!({name}(None).is_none());\n"));
                }
            }
        }
    }
    source.push_str(&format!(
        "#[test] fn shared_handles_not_content_copies() {{ {assertions} }}"
    ));
    common::cargo_test_generated_modules(
        "native_allocation_shapes",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: source,
            tests: "",
        }],
    );
}
