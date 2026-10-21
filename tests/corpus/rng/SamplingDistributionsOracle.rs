//! Independent callers only; sampling algorithms are the generated upstream methods.
use std::{
    any::Any,
    cell::{Cell, RefCell},
    rc::Rc,
};
use translated::*;

struct TraceState {
    word: u64,
    calls: usize,
    trace: Vec<String>,
}
#[derive(Clone)]
struct TraceRng(Rc<RefCell<TraceState>>);
impl TraceRng {
    fn new() -> Self {
        Self(Rc::new(RefCell::new(TraceState {
            word: 0x123456789abcdef0,
            calls: 0,
            trace: vec![],
        })))
    }
    fn word(&self) -> u64 {
        let mut state = self.0.borrow_mut();
        state.calls += 1;
        assert!(state.calls <= 100000, "draw budget exceeded");
        state.word = state.word.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = state.word;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    fn drain(&self) -> String {
        let mut state = self.0.borrow_mut();
        let result = format!("[{}]", state.trace.join(", "));
        state.trace.clear();
        result
    }
}
impl UniformRandomProvider for TraceRng {
    fn next_double(&mut self) -> Result<f64, JavaException> {
        let value = (self.word() >> 11) as f64 * (1.0 / 9007199254740992.0);
        self.0
            .borrow_mut()
            .trace
            .push(format!("d:{}", value.to_bits()));
        Ok(value)
    }
    fn next_int(&mut self) -> Result<i32, JavaException> {
        let value = self.word() as i32;
        self.0.borrow_mut().trace.push(format!("i:{value}"));
        Ok(value)
    }
    fn next_int_int(&mut self, bound: i32) -> Result<i32, JavaException> {
        assert!(bound > 0);
        let value = (self.word() % bound as u64) as i32;
        self.0.borrow_mut().trace.push(format!("b:{bound}:{value}"));
        Ok(value)
    }
    fn next_long(&mut self) -> Result<i64, JavaException> {
        panic!("unexpected nextLong")
    }
    fn next_long_long(&mut self, _: i64) -> Result<i64, JavaException> {
        panic!("unexpected boundedLong")
    }
    fn next_boolean(&mut self) -> Result<bool, JavaException> {
        panic!("unexpected boolean")
    }
    fn next_float(&mut self) -> Result<f32, JavaException> {
        panic!("unexpected float")
    }
    fn next_bytes_byte_array(&self, _: Rc<RefCell<Vec<i8>>>) -> Result<(), JavaException> {
        panic!("unexpected bytes")
    }
    fn next_bytes_byte_array_int_int(
        &self,
        _: Rc<RefCell<Vec<i8>>>,
        _: i32,
        _: i32,
    ) -> Result<(), JavaException> {
        panic!("unexpected bytes")
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn clone_any(&self) -> Box<dyn Any> {
        Box::new(self.clone())
    }
    fn clone_box(&self) -> Box<dyn UniformRandomProvider> {
        Box::new(self.clone())
    }
    fn to_string(&self) -> String {
        "trace".into()
    }
}
fn display(label: &str, text: String) {
    // A mismatch remains an explicit comparator failure while allowing the
    // rest of the numeric/state observations to be collected in one run.
    let result = if text.contains("rng=") && text.ends_with(']') {
        "ok"
    } else {
        "bad"
    };
    println!("{label}.display={result}");
    if result != "ok" {
        eprintln!("{label}: unexpected display {text:?}");
    }
}
fn continuous(label: &str, make: impl FnOnce(&TraceRng) -> Box<dyn ContinuousSampler>) {
    let rng = TraceRng::new();
    let mut sampler = make(&rng);
    println!("{label}.constructor.trace={}", rng.drain());
    for i in 0..24 {
        println!("{label}.double.{i}={:?}", sampler.sample().unwrap());
        println!("{label}.trace.{i}={}", rng.drain());
    }
    display(label, ContinuousSampler::to_string(&*sampler));
}
fn discrete(label: &str, make: impl FnOnce(&TraceRng) -> Box<dyn DiscreteSampler>) {
    let rng = TraceRng::new();
    let mut sampler = make(&rng);
    println!("{label}.constructor.trace={}", rng.drain());
    for i in 0..24 {
        println!("{label}.int.{i}={}", sampler.sample().unwrap());
        println!("{label}.trace.{i}={}", rng.drain());
    }
    display(label, DiscreteSampler::to_string(&*sampler));
}
fn exception<T>(label: &str, expected: &str, result: Result<T, JavaException>) {
    match result {
        Err(error) => {
            assert!(error.is_instance(&[expected]), "{label}: {error:?}");
            println!("{label}={expected}");
        }
        Ok(_) => panic!("{label} did not throw"),
    }
}
#[derive(Clone, Default)]
struct ContinuousFunction(Rc<Cell<i32>>);
impl ContinuousInverseCumulativeProbabilityFunction for ContinuousFunction {
    fn inverse_cumulative_probability(&self, p: f64) -> f64 {
        self.0.set(self.0.get() + 1);
        p * 3.0 - 1.0 + self.0.get() as f64 * 0.125
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn clone_any(&self) -> Box<dyn Any> {
        Box::new(self.clone())
    }
    fn clone_box(&self) -> Box<dyn ContinuousInverseCumulativeProbabilityFunction> {
        Box::new(self.clone())
    }
}
#[derive(Clone, Default)]
struct DiscreteFunction(Rc<Cell<i32>>);
impl DiscreteInverseCumulativeProbabilityFunction for DiscreteFunction {
    fn inverse_cumulative_probability(&self, p: f64) -> i32 {
        self.0.set(self.0.get() + 1);
        (p * 7.0).floor() as i32 + self.0.get() * 100
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn clone_any(&self) -> Box<dyn Any> {
        Box::new(self.clone())
    }
    fn clone_box(&self) -> Box<dyn DiscreteInverseCumulativeProbabilityFunction> {
        Box::new(self.clone())
    }
}
fn helpers() {
    for n in 0..=20 {
        println!("factorial.int.{n}={}", InternalUtils::factorial(n).unwrap());
    }
    exception(
        "factorial.negative",
        "ArrayIndexOutOfBoundsException",
        InternalUtils::factorial(-1),
    );
    for size in [0, 2, 5, 32, 80] {
        let cache = InternalUtils_FactorialLog::create()
            .unwrap()
            .with_cache(size)
            .unwrap();
        for n in [0, 1, 2, 3, 4, 10, 19, 20, 21, 40, 80, 120] {
            println!(
                "factorialLog.{size}.double.{n}={:?}",
                cache.value(n).unwrap()
            );
        }
        let smaller = cache.with_cache(3).unwrap();
        println!(
            "factorialLog.{size}.resized.double.2={:?}",
            smaller.value(2).unwrap()
        );
    }
    exception(
        "factorialLog.negativeCache",
        "NegativeArraySizeException",
        InternalUtils_FactorialLog::create().unwrap().with_cache(-1),
    );
    exception(
        "factorialLog.negativeValue",
        "ArrayIndexOutOfBoundsException",
        InternalUtils_FactorialLog::create()
            .unwrap()
            .with_cache(3)
            .unwrap()
            .value(-1),
    );
    let xs = [
        -1.0,
        0.0,
        1e-8,
        0.1,
        0.5,
        0.75,
        1.0,
        1.5,
        2.0,
        2.5,
        3.0,
        5.0,
        7.5,
        8.0,
        15.0,
        16.0,
        100.0,
        172.0,
        1e6,
        f64::NAN,
        f64::INFINITY,
    ];
    for (i, x) in xs.into_iter().enumerate() {
        println!(
            "logGamma.double.{i}={:?}",
            InternalGamma::log_gamma(x).unwrap()
        );
    }
}
macro_rules! cbox {
    ($value:expr) => {
        Box::new(Rc::new(RefCell::new($value)))
    };
}
fn main() {
    continuous("uniform", |r| {
        cbox!(ContinuousUniformSampler::new(r, -2.0, 5.0))
    });
    continuous("uniform.equal", |r| {
        cbox!(ContinuousUniformSampler::new(r, 7.0, 7.0))
    });
    continuous("uniform.reversed", |r| {
        cbox!(ContinuousUniformSampler::new(r, 5.0, -2.0))
    });
    continuous("uniform.zero", |r| {
        cbox!(ContinuousUniformSampler::new(r, -0.0, 0.0))
    });
    continuous("exponential", |r| {
        cbox!(AhrensDieterExponentialSampler::new(r, 2.5))
    });
    continuous("exponential.zero", |r| {
        cbox!(AhrensDieterExponentialSampler::new(r, 0.0))
    });
    for theta in [0.25, 1.0, 2.0, 10.0] {
        continuous(&format!("gamma.{theta:?}"), |r| {
            cbox!(AhrensDieterMarsagliaTsangGammaSampler::new(r, 2.5, theta))
        });
    }
    continuous("gaussian", |r| {
        cbox!(BoxMullerGaussianSampler::new(r, -1.0, 2.0))
    });
    continuous("gaussian.zero", |r| {
        cbox!(BoxMullerGaussianSampler::new(r, 3.0, 0.0))
    });
    continuous("gaussian.negativeSigma", |r| {
        cbox!(BoxMullerGaussianSampler::new(r, 1.0, -2.0))
    });
    continuous("logNormal", |r| {
        cbox!(BoxMullerLogNormalSampler::new(r, 0.25, 0.5))
    });
    continuous("logNormal.zero", |r| {
        cbox!(BoxMullerLogNormalSampler::new(r, 1.0, 0.0))
    });
    for (a, b) in [(1.0, 1.0), (0.2, 5.0), (5.0, 0.2), (2.0, 3.0), (3.0, 2.0)] {
        continuous(&format!("beta.{a:?}.{b:?}"), |r| {
            cbox!(ChengBetaSampler::new(r, a, b))
        });
    }
    let continuous_function = ContinuousFunction::default();
    continuous("inverseContinuous", |r| {
        cbox!(InverseTransformContinuousSampler::new(
            r,
            Box::new(continuous_function.clone())
        ))
    });
    println!(
        "inverseContinuous.callback.calls={}",
        continuous_function.0.get()
    );
    continuous("inverseContinuous.shared", |r| {
        cbox!(InverseTransformContinuousSampler::new(
            r,
            Box::new(continuous_function.clone())
        ))
    });
    println!(
        "inverseContinuous.shared.callback.calls={}",
        continuous_function.0.get()
    );
    let discrete_function = DiscreteFunction::default();
    discrete("inverseDiscrete", |r| {
        Box::new(InverseTransformDiscreteSampler::new(
            r,
            Box::new(discrete_function.clone()),
        ))
    });
    println!(
        "inverseDiscrete.callback.calls={}",
        discrete_function.0.get()
    );
    discrete("inverseDiscrete.shared", |r| {
        Box::new(InverseTransformDiscreteSampler::new(
            r,
            Box::new(discrete_function.clone()),
        ))
    });
    println!(
        "inverseDiscrete.shared.callback.calls={}",
        discrete_function.0.get()
    );
    continuous("pareto", |r| {
        cbox!(InverseTransformParetoSampler::new(r, 2.0, 3.0))
    });
    continuous("pareto.unit", |r| {
        cbox!(InverseTransformParetoSampler::new(r, 1.0, 1.0))
    });
    discrete("discrete", |r| {
        Box::new(DiscreteUniformSampler::new(r, -7, 13).unwrap())
    });
    discrete("discrete.equal", |r| {
        Box::new(DiscreteUniformSampler::new(r, 4, 4).unwrap())
    });
    discrete("discrete.wide", |r| {
        Box::new(DiscreteUniformSampler::new(r, i32::MIN, 17).unwrap())
    });
    discrete("discrete.full", |r| {
        Box::new(DiscreteUniformSampler::new(r, i32::MIN, i32::MAX).unwrap())
    });
    for mean in [0.1, 1.0, 39.5, 40.0, 80.0, 1000.0] {
        discrete(&format!("poisson.{mean:?}"), |r| {
            Box::new(PoissonSampler::new(r, mean).unwrap())
        });
    }
    for exponent in [0.5, 1.0, 1.000000000001, 2.0] {
        discrete(&format!("zipf.{exponent:?}"), |r| {
            Box::new(RejectionInversionZipfSampler::new(r, 100, exponent).unwrap())
        });
    }
    discrete("zipf.one", |r| {
        Box::new(RejectionInversionZipfSampler::new(r, 1, 1.0).unwrap())
    });
    exception(
        "discrete.invalid",
        "IllegalArgumentException",
        DiscreteUniformSampler::new(&TraceRng::new(), 3, 2),
    );
    exception(
        "poisson.zero",
        "IllegalArgumentException",
        PoissonSampler::new(&TraceRng::new(), 0.0),
    );
    exception(
        "poisson.negative",
        "IllegalArgumentException",
        PoissonSampler::new(&TraceRng::new(), -1.0),
    );
    for (label, elements, exponent) in [
        ("zipf.zeroElements", 0, 1.0),
        ("zipf.negativeElements", -1, 1.0),
        ("zipf.zeroExponent", 100, 0.0),
        ("zipf.negativeExponent", 100, -1.0),
    ] {
        exception(
            label,
            "IllegalArgumentException",
            RejectionInversionZipfSampler::new(&TraceRng::new(), elements, exponent),
        );
    }
    helpers();
    println!("oracle=ok");
}
