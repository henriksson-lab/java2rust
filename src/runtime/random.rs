
/// A pseudo-random number generator bit-compatible with `java.util.Random`.
///
/// It reproduces the JDK's 48-bit linear congruential generator exactly, so a
/// given seed yields the same `nextInt`/`nextLong`/`nextDouble`/... sequence as
/// the Java original. The no-arg constructor seeds deterministically (real Java
/// uses a time/uniquifier-based seed; determinism is preferable for a
/// translation and avoids pulling in `std::time`).
///
/// State lives in `Cell`s so every draw takes `&self` (Java's `nextX()` mutate
/// the generator, but a translated `static final Random` lowers to an immutable
/// `const`/field — interior mutability lets those call sites work without `&mut`).
#[derive(Clone, Debug)]
pub struct JavaRandom {
    seed: std::cell::Cell<i64>,
    /// Cached second value of the Box-Muller pair (`nextNextGaussian` in the
    /// JDK); `Some` when a spare gaussian is available.
    next_next_gaussian: std::cell::Cell<Option<f64>>,
}

const MULTIPLIER: i64 = 0x5DEECE66D;
const ADDEND: i64 = 0xB;
const MASK: i64 = (1 << 48) - 1;

impl Default for JavaRandom {
    fn default() -> Self {
        JavaRandom::new()
    }
}

impl JavaRandom {
    /// `new Random()` — a fixed default seed (deterministic across runs).
    pub fn new() -> Self {
        JavaRandom::new_1(0_i64)
    }

    /// `new Random(long seed)`.
    pub fn new_1<S: Into<i64>>(seed: S) -> Self {
        JavaRandom {
            seed: std::cell::Cell::new(Self::scramble(seed.into())),
            next_next_gaussian: std::cell::Cell::new(None),
        }
    }

    fn scramble(seed: i64) -> i64 {
        (seed ^ MULTIPLIER) & MASK
    }

    /// `setSeed(long)` — re-seed and discard any cached gaussian.
    pub fn set_seed(&self, seed: i64) {
        self.seed.set(Self::scramble(seed));
        self.next_next_gaussian.set(None);
    }

    /// The JDK `next(int bits)` primitive: advance the LCG and return the top
    /// `bits` bits as a (sign-extended) `i32`.
    fn next(&self, bits: u32) -> i32 {
        let s = self.seed.get().wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK;
        self.seed.set(s);
        // Arithmetic right shift of the top bits, matching `(int)(seed >>> (48 - bits))`.
        (s >> (48 - bits)) as i32
    }

    /// `nextInt()` — a uniformly distributed `int`.
    pub fn next_int(&self) -> i32 {
        self.next(32)
    }

    /// `nextInt(int bound)` — uniform in `[0, bound)` using Java's rejection
    /// algorithm. Panics if `bound <= 0`, matching Java's
    /// `IllegalArgumentException`.
    pub fn next_int_bound(&self, bound: i32) -> i32 {
        if bound <= 0 {
            panic!("bound must be positive");
        }
        // Power of two: take the high bits directly.
        if (bound & -bound) == bound {
            return ((bound as i64).wrapping_mul(self.next(31) as i64) >> 31) as i32;
        }
        let mut bits;
        let mut val;
        loop {
            bits = self.next(31);
            val = bits % bound;
            if bits.wrapping_sub(val).wrapping_add(bound - 1) >= 0 {
                break;
            }
        }
        val
    }

    /// `nextLong()` — `((long)next(32) << 32) + next(32)`.
    pub fn next_long(&self) -> i64 {
        let hi = self.next(32) as i64;
        let lo = self.next(32) as i64;
        (hi << 32).wrapping_add(lo)
    }

    /// `nextBoolean()`.
    pub fn next_boolean(&self) -> bool {
        self.next(1) != 0
    }

    /// `nextFloat()` — uniform in `[0, 1)`.
    pub fn next_float(&self) -> f32 {
        self.next(24) as f32 / (1 << 24) as f32
    }

    /// `nextDouble()` — uniform in `[0, 1)`.
    pub fn next_double(&self) -> f64 {
        let hi = (self.next(26) as i64) << 27;
        let lo = self.next(27) as i64;
        (hi + lo) as f64 / (1u64 << 53) as f64
    }

    /// `nextGaussian()` — mean 0, stddev 1, via the polar (Marsaglia) method
    /// with the cached spare value, exactly as the JDK does.
    pub fn next_gaussian(&self) -> f64 {
        if let Some(g) = self.next_next_gaussian.take() {
            return g;
        }
        loop {
            let v1 = 2.0 * self.next_double() - 1.0;
            let v2 = 2.0 * self.next_double() - 1.0;
            let s = v1 * v1 + v2 * v2;
            if s < 1.0 && s != 0.0 {
                let multiplier = (-2.0 * s.ln() / s).sqrt();
                self.next_next_gaussian.set(Some(v2 * multiplier));
                return v1 * multiplier;
            }
        }
    }

    /// `nextBytes(byte[])` — fill `bytes` with random values.
    pub fn next_bytes(&self, bytes: &mut [i8]) {
        let mut i = 0;
        while i < bytes.len() {
            let mut rnd = self.next_int();
            let mut n = std::cmp::min(bytes.len() - i, 4);
            while n > 0 {
                bytes[i] = rnd as i8;
                i += 1;
                rnd >>= 8;
                n -= 1;
            }
        }
    }
}

#[cfg(test)]
mod random_tests {
    use super::JavaRandom;

    #[test]
    fn next_int_known_seed_0() {
        // JDK: new Random(0).nextInt() == -1155484576
        let r = JavaRandom::new_1(0_i64);
        assert_eq!(r.next_int(), -1155484576);
    }

    #[test]
    fn next_int_known_seed_42_sequence() {
        // JDK: new Random(42).nextInt() sequence.
        let r = JavaRandom::new_1(42_i64);
        assert_eq!(r.next_int(), -1170105035);
        assert_eq!(r.next_int(), 234785527);
        assert_eq!(r.next_int(), -1360544799);
    }

    #[test]
    fn next_long_known() {
        // JDK: new Random(0).nextLong() == -4962768465676381896
        let r = JavaRandom::new_1(0_i64);
        assert_eq!(r.next_long(), -4962768465676381896);
    }

    #[test]
    fn next_double_known() {
        // JDK: new Random(0).nextDouble() == 0.730967787376657
        let r = JavaRandom::new_1(0_i64);
        let d = r.next_double();
        assert!((d - 0.730967787376657).abs() < 1e-15, "got {d}");
    }

    #[test]
    fn next_boolean_known() {
        // JDK: new Random(0).nextBoolean() == true
        let r = JavaRandom::new_1(0_i64);
        assert!(r.next_boolean());
    }

    #[test]
    fn next_int_bound_known() {
        // JDK: new Random(0).nextInt(100) sequence == 60, 48, 29
        let r = JavaRandom::new_1(0_i64);
        assert_eq!(r.next_int_bound(100), 60);
        assert_eq!(r.next_int_bound(100), 48);
        assert_eq!(r.next_int_bound(100), 29);
    }

    #[test]
    fn next_float_known() {
        // JDK: new Random(0).nextFloat() == 0.73096776 (f32)
        let r = JavaRandom::new_1(0_i64);
        let f = r.next_float();
        assert!((f - 0.73096776).abs() < 1e-7, "got {f}");
    }

    #[test]
    fn next_gaussian_known() {
        // JDK: new Random(0).nextGaussian() == 0.8025330637390305
        let r = JavaRandom::new_1(0_i64);
        let g = r.next_gaussian();
        assert!((g - 0.8025330637390305).abs() < 1e-12, "got {g}");
    }
}
