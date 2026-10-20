//! Test driver only: all generator logic comes from the untouched translated corpus.
use std::{cell::RefCell, rc::Rc};
use translated::{
    JavaException, MersenneTwister64, NumberFactory, RandomProviderDefaultState,
    RandomProviderState, RestorableUniformRandomProvider, SplitMix64, XorShift1024Star,
};

fn shared<T>(value: T) -> Rc<RefCell<T>> {
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

fn exercise(label: &str, rng: &mut dyn RestorableUniformRandomProvider) {
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

fn state_copies() {
    let initial = shared(vec![1i8, 2, 3, 4]);
    let state = RandomProviderDefaultState::new(initial.clone()).unwrap();
    initial.borrow_mut()[0] = 99;
    let copy = state.get_state().unwrap();
    assert_eq!(copy.borrow()[0], 1, "state constructor retained input");
    copy.borrow_mut()[1] = 99;
    assert_eq!(
        state.get_state().unwrap().borrow()[1],
        2,
        "state getter exposed internal array"
    );
    let longs = shared(vec![0i64, -1, i64::MIN, i64::MAX, 0x0123456789abcdef]);
    let bytes = NumberFactory::make_byte_array_long_array(&longs).unwrap();
    assert_eq!(
        *longs.borrow(),
        *NumberFactory::make_long_array(bytes).unwrap().borrow()
    );
    let ints = vec![0i32, -1, i32::MIN, i32::MAX, 0x01234567];
    let bytes = NumberFactory::make_byte_array_int_array(&ints).unwrap();
    assert_eq!(
        ints,
        *NumberFactory::make_int_array(bytes.borrow().clone())
            .unwrap()
            .borrow()
    );
}

fn main() {
    state_copies();
    for (i, seed) in [0i64, 1, -1, i64::MIN, i64::MAX].into_iter().enumerate() {
        exercise(&format!("split.{i}"), &mut shared(SplitMix64::new(seed)));
        let array_seed = shared(vec![seed, 17, -31, i64::MIN]);
        let mut mt = shared(MersenneTwister64::new(array_seed.clone()).unwrap());
        let mut xs = shared(XorShift1024Star::new(array_seed.clone()).unwrap());
        let mut mt_control =
            shared(MersenneTwister64::new(shared(array_seed.borrow().clone())).unwrap());
        let mut xs_control =
            shared(XorShift1024Star::new(shared(array_seed.borrow().clone())).unwrap());
        array_seed.borrow_mut()[0] ^= 0x55aa55aa55aa55aa;
        for _ in 0..16 {
            assert_eq!(
                mt.next_long().unwrap(),
                mt_control.next_long().unwrap(),
                "MT retained seed input"
            );
            assert_eq!(
                xs.next_long().unwrap(),
                xs_control.next_long().unwrap(),
                "XorShift retained seed input"
            );
        }
        exercise(&format!("mt.{i}"), &mut mt);
        exercise(&format!("xor.{i}"), &mut xs);
    }
    exercise(
        "mt.emptySeed",
        &mut shared(MersenneTwister64::new(shared(Vec::new())).unwrap()),
    );
    println!("oracle=ok");
}
