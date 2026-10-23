//! Invoke the automatically translated client, not replacement algorithms.
trait ContractResult {
    fn code(self) -> i32;
}
impl ContractResult for i32 {
    fn code(self) -> i32 {
        self
    }
}
impl<E: std::fmt::Debug> ContractResult for Result<i32, E> {
    fn code(self) -> i32 {
        self.unwrap()
    }
}
fn main() {
    let code = translated::QuaternionContractsOracle::run().code();
    assert_eq!(code, 0, "translated Quaternion client contract");
    callback_lifetime_contract();
    println!("contract={code}");
    println!("oracle=ok");
}

// Rust representation gate in addition to the independent Java behavioral
// comparison: external callbacks own their Slerp, but contained strategies
// must not retain it forever. Both interpolation strategies are constructed
// and invoked through generated production code; no interpolation is recreated.
fn callback_lifetime_contract() {
    use std::rc::Rc;
    use translated::{Quaternion as Q, QuaternionContractsOracle, Slerp};
    for (name, end) in [
        (
            "linear",
            Q::of_double_double_double_double(1.0, 0.00001, 0.0, 0.0),
        ),
        (
            "spherical",
            Q::of_double_double_double_double(0.0, 1.0, 0.0, 0.0),
        ),
    ] {
        let owner = Slerp::new(Some(Q::__static_ONE()), Some(end)).unwrap();
        let weak = Rc::downgrade(&owner);
        let callback = QuaternionContractsOracle::as_callback(owner.clone());
        drop(owner);
        assert!(
            weak.upgrade().is_some(),
            "{name}: external callback must own its Slerp"
        );
        assert!(callback.apply_result(0.5).unwrap().is_some());
        assert!(callback.apply_result(0.25).unwrap().is_some());
        drop(callback);
        assert!(
            weak.upgrade().is_none(),
            "{name}: contained strategy leaked its Slerp owner"
        );
    }
}
