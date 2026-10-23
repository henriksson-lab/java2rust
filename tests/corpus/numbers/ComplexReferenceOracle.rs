//! Caller-only reference requests. Every numerical operation is generated production.
use std::sync::{Arc, Mutex};
use translated::{Complex as C, JavaException};
type Complex = Arc<Mutex<C>>;
fn apply(operation: &str, a: Complex, b: Option<Complex>) -> Result<Complex, JavaException> {
    match operation {
        "acos" => C::acos_with_outer(a),
        "acosh" => C::acosh_with_outer(a),
        "asinh" => C::asinh_with_outer(a),
        "atanh" => C::atanh_with_outer(a),
        "cosh" => Ok(C::cosh_with_outer(a)),
        "sinh" => Ok(C::sinh_with_outer(a)),
        "tanh" => C::tanh_with_outer(a),
        "exp" => Ok(C::exp_with_outer(a)),
        "log" => C::log_with_outer(a),
        "sqrt" => C::sqrt_with_outer(a),
        "multiply" => C::multiply_complex_with_outer(a, b),
        "divide" => C::divide_complex_with_outer(a, b),
        "pow" => C::pow_complex_with_outer(a, b),
        _ => panic!("unknown reference operation {operation}"),
    }
}
fn main() {
    let path = std::env::args().nth(1).expect("reference requests path");
    for line in std::fs::read_to_string(path).unwrap().lines() {
        let fields = line.split('\t').collect::<Vec<_>>();
        assert!(fields.len() == 7 || fields.len() == 9, "{line}");
        let number = |i: usize| fields[i].parse::<f64>().unwrap();
        let a = C::of_cartesian(number(3), number(4));
        let b = (fields.len() == 9).then(|| C::of_cartesian(number(5), number(6)));
        let result = apply(fields[0], a, b).unwrap();
        println!("reference.{}.{}={:x},{:x}", fields[0], fields[1],
            C::real_with_outer(result.clone()).to_bits(), C::imag_with_outer(result).to_bits());
    }
    println!("oracle=ok");
}
