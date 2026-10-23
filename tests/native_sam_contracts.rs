#[path = "common/mod.rs"]
mod common;

#[test]
fn stored_source_callbacks_share_exception_descriptors_through_constructor_delegation() {
    common::verify_java_fixture_zero_contract(
        "stored_source_callback",
        "storedsourcecallback.StoredSourceCallbackProbe",
        "tests/corpus/numbers/StoredSourceCallbackProbe.java",
        include_str!("corpus/numbers/StoredSourceCallbackProbe.java"),
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn stored_errors() { assert_eq!(StoredSourceCallbackProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn named_source_callbacks_retain_transitively_synchronized_payloads() {
    common::verify_java_fixture_zero_contract(
        "named_global_sam",
        "namedglobalsam.NamedGlobalSamProbe",
        "tests/corpus/numbers/NamedGlobalSamProbe.java",
        include_str!("corpus/numbers/NamedGlobalSamProbe.java"),
        r#"
trait OracleExitCode { fn value(self) -> i32; }
impl OracleExitCode for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> OracleExitCode for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn calls_and_identity() { assert_eq!(NamedGlobalSamProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn source_sam_shared_results_use_the_solved_carrier_in_every_return_branch() {
    common::verify_java_fixture_zero_contract(
        "shared_sam_returns",
        "samreturns.SharedSamReturnProbe",
        "tests/corpus/numbers/SharedSamReturnProbe.java",
        include_str!("corpus/numbers/SharedSamReturnProbe.java"),
        r#"
trait OracleExitCode { fn value(self) -> i32; }
impl OracleExitCode for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> OracleExitCode for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn branches_and_identity() { assert_eq!(SharedSamReturnProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn synchronized_callbacks_lift_captures_and_allow_checked_reentrancy() {
    common::verify_java_fixture_zero_contract(
        "synchronized_sam",
        "samsync.SynchronizedSamProbe",
        "tests/corpus/numbers/SynchronizedSamProbe.java",
        include_str!("corpus/numbers/SynchronizedSamProbe.java"),
        r#"
trait OracleExitCode { fn value(self) -> i32; }
impl OracleExitCode for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> OracleExitCode for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn state_and_reentry() { assert_eq!(SynchronizedSamProbe::run().value(), 0); }
#[test] fn shared_dispatch_is_send_sync_and_local_dispatch_keeps_rc_support() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<__JavaSyncDoubleUnaryOperator>();
    let callback = __JavaSyncDoubleUnaryOperator::from_fn(|value| value + 1.0);
    let alias = callback.clone();
    assert!(java_reference_equals(Some(&callback), Some(&alias)));
    assert_eq!(std::thread::spawn(move || alias.applyAsDouble(4.0)).join().unwrap(), 5.0);
    let state = std::rc::Rc::new(std::cell::Cell::new(0));
    let captured = state.clone();
    let local = DoubleUnaryOperator::from_fn(move |value| {
        captured.set(captured.get() + 1);
        value + captured.get() as f64
    });
    assert_eq!(local.applyAsDouble(4.0), 5.0);
    assert_eq!(state.get(), 1);
}
"#,
    );
}

#[test]
fn native_callbacks_preserve_allocation_and_source_reference_identity() {
    common::verify_java_fixture_zero_contract(
        "native_sam_identity",
        "samidentity.NativeSamIdentityProbe",
        "tests/corpus/numbers/NativeSamIdentityProbe.java",
        include_str!("corpus/numbers/NativeSamIdentityProbe.java"),
        r#"
trait OracleExitCode { fn value(self) -> i32; }
impl OracleExitCode for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> OracleExitCode for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn identity() { assert_eq!(NativeSamIdentityProbe::run().value(), 0); }
#[test] fn callback_tokens_and_hashes_survive_handle_moves_and_clones() {
    use std::hash::{Hash, Hasher};
    #[derive(Default)] struct Bytes(Vec<u8>);
    impl Hasher for Bytes {
        fn finish(&self) -> u64 { 0 }
        fn write(&mut self, bytes: &[u8]) { self.0.extend_from_slice(bytes); }
    }
    let first = DoubleUnaryOperator::from_fn(|value| value + 1.0);
    let alias = first.clone();
    let distinct = DoubleUnaryOperator::from_fn(|value| value + 1.0);
    let moved = Box::new(first);
    assert!(java_reference_equals(Some(&*moved), Some(&alias)));
    assert!(!java_reference_equals(Some(&*moved), Some(&distinct)));
    let mut left = Bytes::default();
    let mut right = Bytes::default();
    moved.hash(&mut left);
    alias.hash(&mut right);
    assert!(!left.0.is_empty(), "hashing must observe the allocation token");
    assert_eq!(left.0, right.0);
}
"#,
    );
}
