//! Caller-only oracle: all algorithms are untouched translator output.
use translated::{JavaException, Primes, SmallPrimes};

fn observe<T: std::fmt::Debug>(label: &str, result: Result<T, JavaException>) {
    match result {
        Ok(value) => println!("{label}={value:?}"),
        Err(error) => {
            assert!(error.is_instance(&["IllegalArgumentException"]), "{error:?}");
            println!("{label}=IllegalArgumentException:{}", error.message().scalar_string().expect("this numeric oracle emits scalar exception messages"));
        }
    }
}

fn public_operations(n: i32) {
    observe(&format!("prime.{n}"), Primes::is_prime(n));
    observe(&format!("next.{n}"), Primes::next_prime(n));
    observe(&format!("factors.{n}"), Primes::prime_factors(n));
}

fn helpers() {
    let (primes, wheel) = SmallPrimes::__static_PRIME_NUMBERS_AND_COPRIME_EQUIVALENCE_CLASSES().unwrap();
    println!("table.primes={:?}", SmallPrimes::__static_PRIMES().lock().unwrap());
    println!("table.last={}", SmallPrimes::__static_PRIMES_LAST());
    println!("wheel.values={:?}", wheel.lock().unwrap());
    println!("wheel.size={}", primes.lock().unwrap().len());
    for p in [2, 3, 5, 7, 11, 13] {
        println!("wheel.contains.{p}={}", primes.lock().unwrap().contains_by(&p, |a, b| a == b));
    }
    for n in [2, 64, 123456, 3673 * 3677, 46337 * 46337, i32::MAX] {
        let mut factors = vec![71];
        let remainder = SmallPrimes::small_trial_division(n, &mut factors).unwrap();
        println!("small.remainder.{n}={remainder}");
        println!("small.factors.{n}={factors:?}");
        println!("trial.{n}={:?}", SmallPrimes::trial_division(n).unwrap());
    }
    for n in [3673, 3673 * 3677, 46337 * 46337, i32::MAX] {
        for bound in [3671, 3673, 46340] {
            let mut factors = vec![71];
            SmallPrimes::bounded_trial_division(n, bound, &mut factors).unwrap();
            println!("bounded.{n}.{bound}={factors:?}");
        }
    }
    for n in [3, 5, 561, 1105, 1729, 2047, 2049, 1373651, 1373653, 1373655,
              25325999, 25326001, 25326003, 321197185, 2147483629, i32::MAX] {
        println!("miller.{n}={}", SmallPrimes::miller_rabin_prime_test(n).unwrap());
    }
}

fn main() {
    for n in -8..=1024 { public_operations(n); }
    for n in [i32::MIN, -1, 2047, 2048, 2049, 1373652, 1373653, 1373654,
              25326000, 25326001, 25326002, 3673 * 3677, 46337 * 46337,
              2147483628, 2147483629, 2147483646, i32::MAX] {
        public_operations(n);
    }
    let mut state: i32 = 123456789;
    for _ in 0..128 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        public_operations(state & i32::MAX);
    }
    helpers();
    public_operations(i32::MAX);
    println!("oracle=ok");
}
