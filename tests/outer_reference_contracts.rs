#[path = "common/mod.rs"]
mod common;

#[test]
fn contained_strategy_keeps_payload_aliases_without_retaining_the_owner_cycle() {
    common::verify_java_fixture_zero_contract(
        "contained_final_field_capture",
        "FinalFieldCaptureProbe",
        "tests/corpus/numbers/FinalFieldCaptureProbe.java",
        include_str!("corpus/numbers/FinalFieldCaptureProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
trait Handle<T> { fn handle(self) -> std::rc::Rc<T>; }
impl<T> Handle<T> for std::rc::Rc<T> { fn handle(self) -> std::rc::Rc<T> { self } }
impl<T,E: std::fmt::Debug> Handle<T> for Result<std::rc::Rc<T>,E> {
    fn handle(self) -> std::rc::Rc<T> { self.unwrap() }
}
trait Callback { fn callback(self) -> IntSupplier; }
impl Callback for IntSupplier { fn callback(self) -> IntSupplier { self } }
impl<E: std::fmt::Debug> Callback for Result<IntSupplier,E> {
    fn callback(self) -> IntSupplier { self.unwrap() }
}
#[test] fn aliasing() { assert_eq!(FinalFieldCaptureProbe::run().value(), 0); }
#[test] fn callback_owns_live_owner_but_does_not_leak_it() {
    let owner = FinalFieldCaptureProbe::create().handle();
    let weak = std::rc::Rc::downgrade(&owner);
    let callback = FinalFieldCaptureProbe::callback(owner.clone()).callback();
    drop(owner);
    assert!(weak.upgrade().is_some(), "external callback must keep its owner alive");
    assert_eq!(callback.getAsInt().value(), 8);
    assert_eq!(callback.getAsInt().value(), 9);
    drop(callback);
    assert!(weak.upgrade().is_none(), "contained strategy must not form an owner cycle");
}
"#,
    );
}

#[test]
fn native_builder_chains_preserve_values_and_retained_allocation() {
    common::verify_java_fixture_zero_contract(
        "native_builder_chain_contract",
        "BuilderChainProbe",
        "tests/corpus/numbers/BuilderChainProbe.java",
        include_str!("corpus/numbers/BuilderChainProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn chain() { assert_eq!(BuilderChainProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn retained_inner_callback_projects_the_live_nullable_outer_field() {
    common::verify_java_fixture_zero_contract(
        "retained_inner_nullable_field",
        "OuterReferenceProbe",
        "tests/corpus/numbers/OuterReferenceProbe.java",
        include_str!("corpus/numbers/OuterReferenceProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn callback() { assert_eq!(OuterReferenceProbe::run().value(), 0); }
"#,
    );
}
