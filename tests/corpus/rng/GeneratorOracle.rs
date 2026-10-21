//! Test driver only: all generator logic comes from the untouched translated corpus.
mod oracle_support;
use oracle_support::{exercise, shared};
use translated::{
    MersenneTwister64, NumberFactory, RandomProviderDefaultState, RestorableUniformRandomProvider,
    SplitMix64, XorShift1024Star,
};

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
