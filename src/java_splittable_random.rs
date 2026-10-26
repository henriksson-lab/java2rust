// JavaUtilSplittableRandom's integer generator: SplitMix state advancement and
// Stafford mixing, with Java's overflow-aware bounded rejection sampling.
// Algorithm reference: OpenJDK java.util.SplittableRandom (JDK 8 and later).
// Cloning this Rust handle retains one Java object; split creates another.
#[derive(Clone, Debug)]
pub struct JavaUtilSplittableRandom {
    state: std::sync::Arc<std::sync::Mutex<JavaSplittableRandomState>>,
}

#[derive(Debug)]
struct JavaSplittableRandomState {
    seed: u64,
    gamma: u64,
}

impl JavaSplittableRandomState {
    fn advance(&mut self) -> u64 {
        self.seed = self.seed.wrapping_add(self.gamma);
        self.seed
    }

    fn draw_int(&mut self) -> i32 {
        let first = self.advance();
        let second = (first ^ (first >> 33)).wrapping_mul(0x62a9_d9ed_7997_05f5);
        ((second ^ (second >> 28)).wrapping_mul(0xcb24_d0a5_c88c_35b3) >> 32) as i32
    }

    fn draw_long(&mut self) -> i64 {
        Self::mix_long(self.advance()) as i64
    }

    fn mix_long(input: u64) -> u64 {
        let first = (input ^ (input >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        let second = (first ^ (first >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        second ^ (second >> 31)
    }

    fn mix_gamma(input: u64) -> u64 {
        let first = (input ^ (input >> 33)).wrapping_mul(0xff51_afd7_ed55_8ccd);
        let second = (first ^ (first >> 33)).wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        let candidate = (second ^ (second >> 33)) | 1;
        if (candidate ^ (candidate >> 1)).count_ones() < 24 {
            candidate ^ 0xaaaa_aaaa_aaaa_aaaa
        } else {
            candidate
        }
    }

    fn bounded_int(&mut self, origin: i32, bound: i32) -> i32 {
        let width = bound.wrapping_sub(origin);
        let mask = width.wrapping_sub(1);
        let first = self.draw_int();
        if width & mask == 0 {
            return (first & mask).wrapping_add(origin);
        }
        if width > 0 {
            let mut candidate = (first as u32 >> 1) as i32;
            loop {
                let remainder = candidate % width;
                if candidate.wrapping_add(mask).wrapping_sub(remainder) >= 0 {
                    return remainder.wrapping_add(origin);
                }
                candidate = (self.draw_int() as u32 >> 1) as i32;
            }
        }
        let mut candidate = first;
        while candidate < origin || candidate >= bound {
            candidate = self.draw_int();
        }
        candidate
    }
}

impl JavaUtilSplittableRandom {
    pub fn java_identity_eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.state, &other.state)
    }
    const GOLDEN_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

    fn with_state(seed: u64, gamma: u64) -> Self {
        Self {
            state: std::sync::Arc::new(std::sync::Mutex::new(JavaSplittableRandomState {
                seed,
                gamma,
            })),
        }
    }

    pub fn new() -> Self {
        static SEEDS: std::sync::OnceLock<std::sync::atomic::AtomicU64> =
            std::sync::OnceLock::new();
        let seeds = SEEDS.get_or_init(|| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            let seed = JavaSplittableRandomState::mix_long(now.as_secs())
                ^ JavaSplittableRandomState::mix_long(now.subsec_nanos() as u64)
                ^ JavaSplittableRandomState::mix_long(std::process::id() as u64);
            std::sync::atomic::AtomicU64::new(seed)
        });
        let seed = seeds.fetch_add(
            Self::GOLDEN_GAMMA.wrapping_mul(2),
            std::sync::atomic::Ordering::Relaxed,
        );
        Self::with_state(
            JavaSplittableRandomState::mix_long(seed),
            JavaSplittableRandomState::mix_gamma(seed.wrapping_add(Self::GOLDEN_GAMMA)),
        )
    }

    pub fn new_0() -> Self {
        Self::new()
    }
    pub fn new_1(seed: i64) -> Self {
        Self::with_state(seed as u64, Self::GOLDEN_GAMMA)
    }

    pub fn next_int(&self) -> i32 {
        self.state
            .lock()
            .expect("Java random state mutex poisoned")
            .draw_int()
    }

    pub fn next_int_result(&self, bound: i32) -> Result<i32, JavaException> {
        if bound <= 0 {
            return Err(JavaException::new(
                "IllegalArgumentException",
                "bound must be positive".to_string(),
            ));
        }
        Ok(self
            .state
            .lock()
            .expect("Java random state mutex poisoned")
            .bounded_int(0, bound))
    }

    pub fn next_int_range_result(&self, origin: i32, bound: i32) -> Result<i32, JavaException> {
        if origin >= bound {
            return Err(JavaException::new(
                "IllegalArgumentException",
                "bound must be greater than origin".to_string(),
            ));
        }
        Ok(self
            .state
            .lock()
            .expect("Java random state mutex poisoned")
            .bounded_int(origin, bound))
    }

    pub fn next_long(&self) -> i64 {
        self.state
            .lock()
            .expect("Java random state mutex poisoned")
            .draw_long()
    }

    pub fn split(&self) -> Self {
        let mut state = self.state.lock().expect("Java random state mutex poisoned");
        let seed = state.draw_long() as u64;
        let gamma = JavaSplittableRandomState::mix_gamma(state.advance());
        Self::with_state(seed, gamma)
    }
}

impl Default for JavaUtilSplittableRandom {
    fn default() -> Self {
        Self::new()
    }
}
impl PartialEq for JavaUtilSplittableRandom {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.state, &other.state)
    }
}
impl Eq for JavaUtilSplittableRandom {}
impl JavaReferenceIdentity for JavaUtilSplittableRandom {
    fn java_reference_identity(&self) -> *const () {
        std::sync::Arc::as_ptr(&self.state) as *const ()
    }
}
impl std::hash::Hash for JavaUtilSplittableRandom {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&std::sync::Arc::as_ptr(&self.state), state)
    }
}
impl std::fmt::Display for JavaUtilSplittableRandom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "java.util.SplittableRandom@{:x}",
            std::sync::Arc::as_ptr(&self.state) as usize
        )
    }
}
