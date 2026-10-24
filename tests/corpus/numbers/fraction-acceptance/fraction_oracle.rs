use std::sync::{Arc, Mutex};
use translated::{Fraction, JavaException, JavaString, NativeOperators};

type Value = Arc<Mutex<Fraction>>;
type Outcome = Result<String, JavaException>;

fn units(text: &[u16]) -> String {
    text.iter().map(|unit| format!("{unit:04x}")).collect()
}
fn emit(label: String, request: impl FnOnce() -> Outcome) {
    match request() {
        Ok(value) => println!("{label}\t{value}"),
        Err(error) => {
            let name = error.get_class().get_name();
            // These pinned exception declarations have no literal '$' identifiers.
            // Match the Java oracle's getSimpleName; full identity is checked separately.
            let simple = name.rsplit(['.', '$']).next().unwrap();
            let message = error
                .get_message()
                .map(|text| units(text.units()))
                .unwrap_or_else(|| "NULL".into());
            println!(
                "{label}\tERR\t{simple}\t{}\t{message}",
                error.is_instance(&["ArithmeticException"])
            );
        }
    }
}
fn view(value: Value) -> Outcome {
    // Call the public canonical-receiver entry points. Never hold a Rust guard
    // across a generated call: Java permits self-aliasing on these operations.
    Ok(format!(
        "{}/{}|{:016x}|{:08x}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        Fraction::get_numerator_with_outer(value.clone()),
        Fraction::get_denominator_with_outer(value.clone()),
        Fraction::double_value_with_outer(value.clone()).to_bits(),
        Fraction::float_value_with_outer(value.clone()).to_bits(),
        Fraction::int_value_with_outer(value.clone()),
        Fraction::long_value_with_outer(value.clone())?,
        Fraction::byte_value_with_outer(value.clone()),
        Fraction::short_value_with_outer(value.clone()),
        Fraction::signum_with_outer(value.clone()),
        Fraction::is_zero_with_outer(value.clone())?,
        Fraction::is_one_with_outer(value.clone())?,
        Fraction::hash_code_with_outer(value.clone()),
        units(Fraction::to_string_with_outer(value)?.units())
    ))
}
fn result(input: &Value, output: Value) -> Outcome {
    let same = Arc::ptr_eq(input, &output);
    Ok(format!("{}|same={same}", view(output)?))
}
fn main() {
    let pairs = [
        (0, 1),
        (1, 1),
        (-1, 1),
        (2, 3),
        (-2, 3),
        (2, -3),
        (-2, -3),
        (i32::MIN, 1),
        (1, i32::MIN),
        (i32::MIN, i32::MIN),
        (i32::MAX, 2),
        (0, -7),
        (1, 0),
        (0, 0),
        (i32::MAX, -i32::MAX),
        (i32::MIN, -1),
        (i32::MIN, -2),
        (i32::MIN, i32::MIN + 1),
    ];
    for (i, (n, d)) in pairs.into_iter().enumerate() {
        emit(format!("of-pair:{i}"), || view(Fraction::of_int_int(n, d)?));
        emit(format!("of-int:{i}"), || view(Fraction::of_int(n)));
        let Ok(value) = Fraction::of_int_int(n, d) else {
            continue;
        };
        for op in 0..6 {
            emit(format!("unary:{i}:{op}"), || {
                let output = match op {
                    0 => Fraction::abs_with_outer(value.clone())?,
                    1 => Fraction::negate_with_outer(value.clone())?,
                    2 => Fraction::reciprocal_with_outer(value.clone())?,
                    3 => Fraction::zero_with_outer(value.clone()),
                    4 => Fraction::one_with_outer(value.clone()),
                    _ => Fraction::pow_with_outer(value.clone(), 1)?,
                };
                result(&value, output)
            });
        }
        for exponent in [-3, -1, 0, 1, 2, 7, i32::MIN] {
            emit(format!("pow:{i}:{exponent}"), || {
                result(&value, Fraction::pow_with_outer(value.clone(), exponent)?)
            });
        }
        for arg in [i32::MIN, -2, -1, 0, 1, 2, i32::MAX] {
            for op in 0..4 {
                emit(format!("integer:{i}:{arg}:{op}"), || {
                    let output = match op {
                        0 => Fraction::add_int_with_outer(value.clone(), arg)?,
                        1 => Fraction::subtract_int_with_outer(value.clone(), arg)?,
                        2 => Fraction::multiply_int_with_outer(value.clone(), arg)?,
                        _ => Fraction::divide_int_with_outer(value.clone(), arg)?,
                    };
                    result(&value, output)
                });
            }
        }
        let others = [
            value.clone(),
            Fraction::of_int_int(n, d).unwrap(),
            Fraction::__static_ZERO(),
            Fraction::__static_ONE(),
            Fraction::of_int_int(-2, 3).unwrap(),
        ];
        for (j, other) in others.into_iter().enumerate() {
            for op in 0..6 {
                emit(format!("binary:{i}:{j}:{op}"), || {
                    let output = match op {
                        0 => Fraction::add_fraction_with_outer(value.clone(), other.clone())?,
                        1 => Fraction::subtract_fraction_with_outer(value.clone(), other.clone())?,
                        2 => Fraction::multiply_fraction_with_outer(value.clone(), other.clone())?,
                        3 => Fraction::divide_fraction_with_outer(value.clone(), other.clone())?,
                        4 => {
                            return Ok(Fraction::compare_to_with_outer(
                                value.clone(),
                                other.clone(),
                            )
                            .to_string())
                        }
                        _ => {
                            return Ok(Fraction::equals_with_outer(
                                value.clone(),
                                Some(Box::new(other.clone())),
                            )?
                            .to_string())
                        }
                    };
                    result(&value, output)
                });
            }
        }
        emit(format!("equals-null:{i}"), || {
            Ok(Fraction::equals_with_outer(value.clone(), None)?.to_string())
        });
        emit(format!("equals-other:{i}"), || {
            Ok(Fraction::equals_with_outer(
                value.clone(),
                Some(Box::new(JavaString::from("fraction"))),
            )?
            .to_string())
        });
        let mut operators: Box<dyn NativeOperators<Value>> = value.clone().into();
        for op in 0..12 {
            emit(format!("interface:{i}:{op}"), || {
                let output = match op {
                    0 => operators.add(value.clone())?,
                    1 => operators.subtract(value.clone())?,
                    2 => operators.multiply(value.clone())?,
                    3 => operators.divide(value.clone())?,
                    4 => operators.pow(-1)?,
                    5 => operators.multiply_int(2)?,
                    6 => operators.negate()?,
                    7 => operators.reciprocal()?,
                    8 => operators.zero(),
                    9 => operators.one(),
                    10 => return Ok(operators.is_zero()?.to_string()),
                    _ => return Ok(operators.is_one()?.to_string()),
                };
                result(&value, output)
            });
        }
        emit(format!("unchanged:{i}"), || view(value.clone()));
    }
    for (i, value) in [
        -0.0,
        0.0,
        0.5,
        -1.25,
        std::f64::consts::PI,
        1.0 / 3.0,
        1e9,
        f64::NAN,
        f64::INFINITY,
    ]
    .into_iter()
    .enumerate()
    {
        emit(format!("from:{i}"), || view(Fraction::from_double(value)?));
        emit(format!("from-epsilon:{i}"), || {
            view(Fraction::from_double_double_int(value, 1e-8, 100)?)
        });
        emit(format!("from-limit:{i}"), || {
            view(Fraction::from_double_int(value, 100)?)
        });
        emit(format!("from-iterations:{i}"), || {
            view(Fraction::from_double_double_int(value, 0.0, 1)?)
        });
    }
    for (i, text) in [
        "0",
        "1",
        "-2 / 3",
        " 4 / -6 ",
        "2147483647 / 2",
        "1/0",
        "x",
        "1/2/3",
        "",
        "\u{0661} / \u{ff12}",
    ]
    .into_iter()
    .enumerate()
    {
        emit(format!("parse:{i}"), || {
            view(Fraction::parse(&JavaString::from(text))?)
        });
    }
    emit("constant-zero".into(), || view(Fraction::__static_ZERO()));
    emit("constant-one".into(), || view(Fraction::__static_ONE()));
    emit("invalid-denominator-limit".into(), || {
        view(Fraction::from_double_int(1.0, 0)?)
    });
    emit("invalid-epsilon-nan".into(), || {
        view(Fraction::from_double_double_int(1.0, f64::NAN, 100)?)
    });
    emit("invalid-epsilon-negative".into(), || {
        view(Fraction::from_double_double_int(1.0, -1.0, 100)?)
    });
    emit("invalid-iterations".into(), || {
        view(Fraction::from_double_double_int(1.0, 1e-8, 0)?)
    });
    for (i, value) in [
        0.75000000001455192,
        1e10,
        -1e10,
        -43979.60679604749,
        (1.0 + 5.0_f64.sqrt()) / 2.0,
    ]
    .into_iter()
    .enumerate()
    {
        emit(format!("conversion-overflow:{i}"), || {
            view(Fraction::from_double_double_int(value, 1e-12, 25)?)
        });
        emit(format!("denominator-overflow:{i}"), || {
            view(Fraction::from_double_int(value, 1000)?)
        });
    }
}
