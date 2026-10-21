//! Test-only caller of the ten generated native source32 implementations.
mod oracle_support;
use oracle_support::{exercise, shared};
use std::{cell::RefCell, rc::Rc};
use translated::{
    ISAACRandom, KISSRandom, MersenneTwister, MultiplyWithCarry256,
    RestorableUniformRandomProvider, Well1024a, Well19937a, Well19937c, Well44497a, Well44497b,
    Well512a,
};

fn generator(kind: usize, seed: Rc<RefCell<Vec<i32>>>) -> Box<dyn RestorableUniformRandomProvider> {
    match kind {
        0 => Box::new(shared(ISAACRandom::new(seed).unwrap())),
        1 => Box::new(shared(KISSRandom::new(seed).unwrap())),
        2 => Box::new(shared(MersenneTwister::new(seed).unwrap())),
        3 => Box::new(shared(MultiplyWithCarry256::new(seed).unwrap())),
        4 => Box::new(shared(Well512a::new(seed).unwrap())),
        5 => Box::new(shared(Well1024a::new(seed).unwrap())),
        6 => Box::new(shared(Well19937a::new(seed).unwrap())),
        7 => Box::new(shared(Well19937c::new(seed).unwrap())),
        8 => Box::new(shared(Well44497a::new(seed).unwrap())),
        9 => Box::new(shared(Well44497b::new(seed).unwrap())),
        _ => panic!("unknown test generator {kind}"),
    }
}

fn main() {
    let names = [
        "isaac",
        "kiss",
        "mt32",
        "mwc",
        "well512a",
        "well1024a",
        "well19937a",
        "well19937c",
        "well44497a",
        "well44497b",
    ];
    let large = (0i32..1536)
        .map(|i| (0x9e3779b9u32 as i32).wrapping_mul(i + 1))
        .collect();
    let seeds = [
        vec![],
        vec![0],
        vec![1],
        vec![-1],
        vec![i32::MIN, i32::MAX],
        vec![1, 17, -31, i32::MIN, i32::MAX],
        large,
    ];
    for (kind, name) in names.iter().enumerate() {
        for (s, original) in seeds.iter().enumerate() {
            let label = format!("{name}.{s}");
            let seed = shared(original.clone());
            let mut rng = generator(kind, seed.clone());
            let mut control = generator(kind, shared(original.clone()));
            if !original.is_empty() {
                seed.borrow_mut()[0] ^= 0x55aa55aa;
            }
            for _ in 0..16 {
                assert_eq!(
                    rng.next_int().unwrap(),
                    control.next_int().unwrap(),
                    "{label} retained seed input"
                );
            }
            let mut hash = 0x0123456789abcdefi64;
            for _ in 0..2048 {
                hash = hash.rotate_left(7) ^ rng.next_int().unwrap() as i64;
            }
            println!("{label}.warmupChecksum={hash}");
            exercise(&label, rng.as_mut());
        }
    }
    println!("oracle=ok");
}
