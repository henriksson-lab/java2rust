//! Shared test caller, never generator implementation.
use std::{cell::RefCell, rc::Rc};
use translated::{
    JavaException, RandomProviderDefaultState, RandomProviderState, RestorableUniformRandomProvider,
};

pub(crate) fn shared<T>(value: T) -> Rc<RefCell<T>> {
    Rc::new(RefCell::new(value))
}

#[derive(Clone)]
struct ForeignState;
impl RandomProviderState for ForeignState {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn clone_any(&self) -> Box<dyn std::any::Any> {
        Box::new(self.clone())
    }
    fn clone_box(&self) -> Box<dyn RandomProviderState> {
        Box::new(self.clone())
    }
}

fn expect_exception<T>(value: Result<T, JavaException>, kind: &str) {
    match value {
        Err(error) => assert!(error.is_instance(&[kind]), "expected {kind}, got {error:?}"),
        Ok(_) => panic!("expected {kind}"),
    }
}

pub(crate) fn exercise(label: &str, rng: &mut dyn RestorableUniformRandomProvider) {
    for i in 0..64 {
        println!("{label}.long.{i}={}", rng.next_long().unwrap());
    }
    let saved = rng.save_state().unwrap();
    let replay: Vec<_> = (0..16).map(|_| rng.next_long().unwrap()).collect();
    rng.restore_state(saved.as_ref()).unwrap();
    for value in replay {
        assert_eq!(value, rng.next_long().unwrap(), "{label} restore replay");
    }
    for i in 0..16 {
        println!("{label}.int.{i}={}", rng.next_int().unwrap());
        println!(
            "{label}.boundedInt.{i}={}",
            rng.next_int_int(i + 1).unwrap()
        );
        println!(
            "{label}.boundedLong.{i}={}",
            rng.next_long_long((1i64 << i) + 1).unwrap()
        );
        println!("{label}.bool.{i}={}", rng.next_boolean().unwrap());
        // The comparison checks IEEE values, independently of decimal spelling.
        println!("{label}.float.{i}={}", rng.next_float().unwrap());
        println!("{label}.double.{i}={}", rng.next_double().unwrap());
    }
    for length in 0..=17 {
        let bytes = shared(vec![99i8; length + 4]);
        rng.next_bytes_byte_array_int_int(bytes.clone(), 2, length as i32)
            .unwrap();
        let bytes = bytes.borrow();
        assert_eq!(
            [bytes[0], bytes[1], bytes[length + 2], bytes[length + 3]],
            [99; 4]
        );
        println!("{label}.bytes.{length}={:?}", &*bytes);
    }
    let whole = shared(vec![0i8; 19]);
    rng.next_bytes_byte_array(whole.clone()).unwrap();
    println!("{label}.wholeBytes={:?}", &*whole.borrow());
    expect_exception(rng.next_int_int(0), "IllegalArgumentException");
    expect_exception(rng.next_long_long(-1), "IllegalArgumentException");
    expect_exception(
        rng.next_bytes_byte_array_int_int(whole, -1, 1),
        "IndexOutOfBoundsException",
    );
    expect_exception(rng.restore_state(&ForeignState), "IllegalArgumentException");
    let malformed = Rc::new(RandomProviderDefaultState::new(shared(Vec::new())).unwrap());
    expect_exception(rng.restore_state(&malformed), "IllegalArgumentException");
}
