//! Caller-only oracle: all solver algorithms and validation are generated.
use translated::{BrentSolver, JavaException};

fn bits(x: f64) -> String { format!("{:x}", x.to_bits()) }

fn run(label: &str, function: i32, min: f64, initial: f64, max: f64, explicit: bool, fail_at: usize) {
    run_with_accuracy(label, function, min, initial, max, explicit, fail_at, 1e-14, 1e-14, 0.0);
}

fn run_with_accuracy(label: &str, function: i32, min: f64, initial: f64, max: f64,
                     explicit: bool, fail_at: usize, relative: f64, absolute: f64, value_accuracy: f64) {
    let solver = BrentSolver::new(relative, absolute, value_accuracy);
    let mut trace = Vec::new();
    let mut callback = |x: f64| -> Result<f64, JavaException> {
        trace.push(bits(x));
        if trace.len() == fail_at || trace.len() > 1024 {
            return Err(JavaException::new("IllegalStateException", format!("callback-{}", trace.len())));
        }
        Ok(match function {
            0 => x * x - 2.0,
            1 => 1e-200 * (x - 0.1),
            2 => -1e-200 * (x - 0.1),
            3 => x.cos() - x,
            4 => (x - 0.25) * (x - 0.25) * (x - 0.25),
            5 => x,
            6 => x * x + 1.0,
            7 => f64::NAN,
            8 => x - 1.0,
            _ => panic!("unknown test callback"),
        })
    };
    let result = if explicit {
        solver.find_root_double_unary_operator_double_double_double(&mut callback, min, initial, max)
    } else {
        solver.find_root_double_unary_operator_double_double(&mut callback, min, max)
    };
    match result {
        Ok(root) => println!("{label}.root.bits={}", bits(root)),
        Err(error) => println!("{label}.error={}:{}", error.class_name().rsplit('.').next().unwrap(), error.message().scalar_string().expect("this solver oracle emits scalar exception messages")),
    }
    drop(callback);
    println!("{label}.calls={}", trace.len());
    println!("{label}.trace={}", trace.join(","));
}

fn main() {
    run("sqrt.default", 0, 0.0, 0.0, 2.0, false, 0);
    for initial in [0.0, 0.25, 0.9, 1.9, 2.0] {
        run(&format!("sqrt.initial.{}", bits(initial)), 0, 0.0, initial, 2.0, true, 0);
    }
    run("tiny.positive", 1, 0.0, 0.0, 1.0, false, 0);
    run("tiny.negative", 2, 0.0, 0.0, 1.0, false, 0);
    run("cosine", 3, 0.0, 0.0, 1.0, false, 0);
    run("cubic", 4, -1.0, 0.9, 2.0, true, 0);
    run("endpoint.low", 8, 1.0, 1.5, 2.0, true, 0);
    run("endpoint.high", 8, 0.0, 0.25, 1.0, true, 0);
    run("initial.root", 8, 0.0, 1.0, 2.0, true, 0);
    run("zero.negative", 5, -0.0, 0.0, 0.0, false, 0);
    run("zero.positive", 5, 0.0, 0.0, 0.0, false, 0);
    run("huge", 5, -f64::MAX, 0.0, f64::MAX, false, 0);
    run("subnormal.root", 5, 0.0, 0.0, f64::from_bits(1), false, 0);
    run("subnormal.singleton", 5, f64::from_bits(1), 0.0, f64::from_bits(1), false, 0);
    run("invalid.interval", 0, 2.0, 0.0, 1.0, false, 0);
    run("invalid.initial.low", 0, 0.0, -1.0, 2.0, true, 0);
    run("invalid.initial.high", 0, 0.0, 3.0, 2.0, true, 0);
    run("invalid.bracket", 6, -2.0, 0.0, 2.0, false, 0);
    run("nan.function", 7, -1.0, 0.0, 1.0, false, 0);
    run("nan.bound", 5, f64::NAN, 0.0, 1.0, false, 0);
    run("infinite.bound", 5, -1.0, 0.0, f64::INFINITY, false, 0);
    for call in [1, 2, 3, 5] { run(&format!("throw.default.{call}"), 0, 0.0, 0.0, 2.0, false, call); }
    for call in [1, 2, 3, 5] { run(&format!("throw.explicit.{call}"), 0, 0.0, 0.25, 2.0, true, call); }
    run_with_accuracy("accuracy.absolute", 0, 0.0, 0.0, 2.0, false, 0, 1e-12, 1e-2, 0.0);
    run_with_accuracy("accuracy.relative", 0, 0.0, 0.0, 2.0, false, 0, 1e-2, 1e-12, 0.0);
    run_with_accuracy("accuracy.zero.relative", 0, 0.0, 0.0, 2.0, false, 0, 0.0, 1e-12, 0.0);
    run_with_accuracy("accuracy.function.initial", 8, 0.0, 0.9, 2.0, true, 0, 1e-14, 1e-14, 0.2);
    run_with_accuracy("accuracy.function.low", 8, 0.9, 1.8, 2.0, true, 0, 1e-14, 1e-14, 0.2);
    run_with_accuracy("accuracy.function.high", 8, -2.0, 0.0, 1.1, true, 0, 1e-14, 1e-14, 0.2);
    let solver = BrentSolver::new(1e-14, 1e-14, 0.0);
    let calls = std::cell::Cell::new(0);
    let mut callback = |x| { calls.set(calls.get() + 1); Ok(x - 0.75) };
    println!("repeat.first.bits={}", bits(solver.find_root_double_unary_operator_double_double(&mut callback, 0.0, 1.0).unwrap()));
    println!("repeat.first.calls={}", calls.get());
    let alias = &mut callback;
    println!("repeat.alias.bits={}", bits(solver.find_root_double_unary_operator_double_double_double(alias, 0.0, 0.9, 1.0).unwrap()));
    println!("repeat.alias.calls={}", calls.get());
    println!("oracle=ok");
}
