#[path = "common/mod.rs"]
mod common;

#[test]
fn nullable_covariant_self_returns_preserve_null_identity_and_exceptions() {
    check_source(
        include_str!("fixtures/nullable_virtual_self_dispatch.java"),
        "nullable_virtual_self_dispatch",
        "NullableVirtualSelfDispatch",
        "",
    );
}

#[test]
fn inlined_inherited_methods_flush_iterator_mutations_on_return_and_throw() {
    check_source(
        include_str!("fixtures/shared_iterator_cleanup.java"),
        "shared_iterator_cleanup",
        "SharedIteratorCleanup",
        "",
    );
}

#[test]
fn inherited_fluent_self_calls_keep_the_specialized_receiver() {
    check_source(
        include_str!("fixtures/fluent_virtual_self_dispatch.java"),
        "fluent_virtual_self_dispatch",
        "FluentVirtualSelfDispatch",
        "",
    );
}

#[test]
fn inherited_bodies_dispatch_current_self_to_covariant_overrides() {
    check_source(
        include_str!("fixtures/virtual_self_dispatch.java"),
        "virtual_self_dispatch",
        "VirtualSelfDispatch",
        "",
    );
}

#[test]
fn inherited_bodies_dispatch_current_self_in_the_child_generic_frame() {
    check_source(
        include_str!("fixtures/generic_virtual_self_dispatch.java"),
        "generic_virtual_self_dispatch",
        "GenericVirtualSelfDispatch",
        "",
    );
}

#[test]
fn inherited_self_calls_keep_the_canonical_receiver() {
    check_source(
        include_str!("fixtures/inherited_self_calls.java"),
        "inherited_self_calls",
        "InheritedSelfCalls",
        "",
    );
}

#[test]
fn inherited_generic_self_calls_keep_the_specialized_receiver() {
    check_source(
        include_str!("fixtures/inherited_generic_self_calls.java"),
        "inherited_generic_self_calls",
        "InheritedGenericSelfCalls",
        r#"
fn clone_carrier_without_cloning_the_type_argument<T>(value: &GenericCallBaseValue<T>) -> GenericCallBaseValue<T> {
    std::clone::Clone::clone(value)
}
"#,
    );
}

fn check_source(source: &str, crate_name: &str, main_type: &str, extra_proof: &str) {
    let translation = new_java2rust::translate_source(source);
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    let rust = format!(
        "{}\ntype ProbeMain = {main_type};\n{extra_proof}",
        translation.rust
    );
    common::cargo_test_generated_modules(
        crate_name,
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn escaped_view_observes_the_original_child() {
    assert_eq!(ProbeMain::run().value(), 0);
}
"#,
        }],
    );
}
