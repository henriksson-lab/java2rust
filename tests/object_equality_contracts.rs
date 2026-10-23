#[path = "common/mod.rs"]
mod common;

#[test]
fn native_sam_classes_keep_checked_object_display_and_virtual_hash_contracts() {
    common::verify_java_fixture_zero_contract(
        "native_object_display",
        "objectcontracts.NativeObjectDisplayProbe",
        "tests/corpus/numbers/NativeObjectDisplayProbe.java",
        include_str!("corpus/numbers/NativeObjectDisplayProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn display() { assert_eq!(NativeObjectDisplayProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn interface_default_equals_uses_checked_canonical_virtual_dispatch() {
    common::verify_java_fixture_zero_contract(
        "checked_interface_equals",
        "CheckedInterfaceEqualsProbe",
        "tests/corpus/numbers/CheckedInterfaceEqualsProbe.java",
        include_str!("corpus/numbers/CheckedInterfaceEqualsProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn dispatch() { assert_eq!(CheckedInterfaceEqualsProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn generic_constructor_field_and_erased_return_share_a_reference_contract() {
    common::verify_java_fixture_zero_contract(
        "generic_erased_field",
        "GenericErasedFieldProbe",
        "tests/corpus/numbers/GenericErasedFieldProbe.java",
        include_str!("corpus/numbers/GenericErasedFieldProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn reference() { assert_eq!(GenericErasedFieldProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn mutable_equals_uses_canonical_carriers_for_inherited_trait_forwarding() {
    common::verify_java_fixture_zero_contract(
        "mutable_equals_forwarding",
        "MutableEqualsForwardingProbe",
        "tests/corpus/numbers/MutableEqualsForwardingProbe.java",
        include_str!("corpus/numbers/MutableEqualsForwardingProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn forwarding() { assert_eq!(MutableEqualsForwardingProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn checked_object_protocols_survive_multi_module_runtime_assembly() {
    common::cargo_run_translated_batch_sources(
        "checked_object_protocol_assembly",
        &[
            new_java2rust::BatchSource::new(
                "ErasedObjectEqualityProbe.java",
                include_str!("corpus/numbers/ErasedObjectEqualityProbe.java"),
            ),
            new_java2rust::BatchSource::new(
                "NullableEqualsArgumentProbe.java",
                include_str!("corpus/numbers/NullableEqualsArgumentProbe.java"),
            ),
        ],
        r#"
use translated::{ErasedObjectEqualityProbe, NullableEqualsArgumentProbe};
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
fn main() {
    assert_eq!(ErasedObjectEqualityProbe::run().value(), 0);
    assert_eq!(NullableEqualsArgumentProbe::run().value(), 0);
}
"#,
    );
}

#[test]
fn erased_object_dispatch_keeps_heterogeneous_null_mutation_exception_and_identity_contracts() {
    common::verify_java_fixture_zero_contract(
        "erased_object_equality",
        "ErasedObjectEqualityProbe",
        "tests/corpus/numbers/ErasedObjectEqualityProbe.java",
        include_str!("corpus/numbers/ErasedObjectEqualityProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn protocol() { assert_eq!(ErasedObjectEqualityProbe::run().value(), 0); }
#[test] fn erased_carrier_wrappers_are_not_new_java_objects() {
    let value = std::rc::Rc::new(std::cell::RefCell::new(ErasedEqualsReentrant::new()));
    let left: std::rc::Rc<Box<dyn std::any::Any>> = std::rc::Rc::new(Box::new(std::rc::Rc::clone(&value)));
    let right: std::sync::Arc<Box<dyn std::any::Any>> = std::sync::Arc::new(Box::new(std::rc::Rc::clone(&value)));
    assert!(java_erased_any_equals_result(&left, Some(&right)).unwrap());
    assert_eq!(value.borrow().calls, 1);
    assert_eq!(java_erased_reference_identity(&left), Some(std::rc::Rc::as_ptr(&value) as *const ()));
    assert_eq!(java_erased_reference_identity(&right), Some(std::rc::Rc::as_ptr(&value) as *const ()));
}
#[test] fn null_argument_does_not_hide_an_unregistered_receiver() {
    struct UnknownErasedPayload;
    let error = std::panic::catch_unwind(|| java_erased_any_equals_result(&UnknownErasedPayload, None))
        .expect_err("an unknown receiver is not evidence for the native null result");
    let message = error.downcast_ref::<&str>().copied()
        .or_else(|| error.downcast_ref::<String>().map(String::as_str)).unwrap();
    assert!(message.contains("unsupported Java erased object equals"));
    assert!(!java_erased_any_equals_result(&"key".to_string(), None).unwrap());
    assert!(!java_erased_any_equals_result(&1i32, None).unwrap());
}
"#,
    );
}

#[test]
fn erased_generic_reference_copies_keep_identity_and_shared_state() {
    common::verify_java_fixture_zero_contract(
        "generic_reference_copy",
        "GenericReferenceCopyProbe",
        "tests/corpus/numbers/GenericReferenceCopyProbe.java",
        include_str!("corpus/numbers/GenericReferenceCopyProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn copies() { assert_eq!(GenericReferenceCopyProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn object_equals_accepts_nullable_arguments_across_source_calls() {
    common::verify_java_fixture_zero_contract(
        "nullable_equals_argument",
        "NullableEqualsArgumentProbe",
        "tests/corpus/numbers/NullableEqualsArgumentProbe.java",
        include_str!("corpus/numbers/NullableEqualsArgumentProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn nullable() { assert_eq!(NullableEqualsArgumentProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn generic_object_equals_dispatches_nullable_mutating_inherited_and_throwing_overrides() {
    common::verify_java_fixture_zero_contract(
        "generic_object_equality",
        "GenericObjectEqualityOracle",
        "tests/corpus/numbers/GenericObjectEqualityOracle.java",
        include_str!("corpus/numbers/GenericObjectEqualityOracle.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn protocol() { assert_eq!(GenericObjectEqualityOracle::run().value(), 0); }
"#,
    );
}
