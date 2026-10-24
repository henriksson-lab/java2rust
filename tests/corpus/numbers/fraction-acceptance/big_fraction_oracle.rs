use java_math_runtime::{BigDecimal, BigInteger, RoundingMode};
use std::sync::Arc;
use translated::{BigFraction as Fraction, JavaException, JavaString, NativeOperators};

type Value = Arc<Fraction>;
type Outcome = Result<String, JavaException>;
fn units(text: &[u16]) -> String {
    text.iter().map(|unit| format!("{unit:04x}")).collect()
}
fn emit(label: String, request: impl FnOnce() -> Outcome) {
    match request() {
        Ok(value) => println!("{label}\t{value}"),
        Err(error) => {
            let name = error.get_class().get_name();
            // Pinned exception identifiers have no literal '$'; Java uses getSimpleName.
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
    Ok(format!(
        "{}/{}|{:016x}|{:08x}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        value.get_numerator().unwrap(),
        value.get_denominator().unwrap(),
        value.double_value()?.to_bits(),
        value.float_value()?.to_bits(),
        value.int_value()?,
        value.long_value()?,
        value.byte_value()?,
        value.short_value()?,
        value.signum()?,
        value.is_zero()?,
        value.is_one()?,
        value.hash_code()?,
        value.get_numerator_as_int()?,
        value.get_numerator_as_long()?,
        value.get_denominator_as_int()?,
        value.get_denominator_as_long()?,
        units(Fraction::to_string(&value)?.units())
    ))
}
fn result(input: &Value, output: Value) -> Outcome {
    let same = Arc::ptr_eq(input, &output);
    Ok(format!("{}|same={same}", view(output)?))
}
fn decimal(value: BigDecimal) -> Outcome {
    Ok(format!("{}|{}", value, value.scale()))
}
fn integer(value: &str) -> BigInteger {
    BigInteger::new(value.encode_utf16().collect::<Vec<_>>()).unwrap()
}
fn main() {
    let pairs = [
        ("0", "1"),
        ("1", "1"),
        ("-1", "1"),
        ("2", "3"),
        ("-2", "3"),
        ("2", "-3"),
        ("9223372036854775808", "-7"),
        ("123456789012345678901234567890", "998877665544332211"),
        (
            "1",
            "10000000000000000000000000000000000000000000000000000000000",
        ),
        ("1", "0"),
    ];
    for (i, (n, d)) in pairs.into_iter().enumerate() {
        let n = integer(n);
        let d = integer(d);
        emit(format!("of-big-pair:{i}"), || {
            view(Fraction::of_big_integer_big_integer(n.clone(), d.clone())?)
        });
        emit(format!("of-big:{i}"), || {
            view(Fraction::of_big_integer(n.clone()))
        });
        emit(format!("of-int:{i}"), || {
            view(Fraction::of_int(n.int_value()))
        });
        emit(format!("of-long:{i}"), || {
            view(Fraction::of_long(n.long_value()))
        });
        emit(format!("of-int-pair:{i}"), || {
            view(Fraction::of_int_int(n.int_value(), d.int_value())?)
        });
        emit(format!("of-long-pair:{i}"), || {
            view(Fraction::of_long_long(n.long_value(), d.long_value())?)
        });
        let Ok(value) = Fraction::of_big_integer_big_integer(n.clone(), d.clone()) else {
            continue;
        };
        emit(format!("retained-integer:{i}"), || {
            Ok(Fraction::of_big_integer(n.clone())
                .get_numerator()
                .unwrap()
                .java_identity_eq(&n)
                .to_string())
        });
        for op in 0..5 {
            emit(format!("unary:{i}:{op}"), || {
                let output = match op {
                    0 => Fraction::abs_with_outer(value.clone())?,
                    1 => Fraction::negate_with_outer(value.clone())?,
                    2 => Fraction::reciprocal_with_outer(value.clone())?,
                    3 => value.zero(),
                    _ => value.one(),
                };
                result(&value, output)
            });
        }
        for exponent in [-3, -1, 0, 1, 2, 7] {
            emit(format!("pow:{i}:{exponent}"), || {
                result(&value, Fraction::pow_with_outer(value.clone(), exponent)?)
            });
        }
        for arg in [-2, 0, 1, 3] {
            for kind in 0..4 {
                let other = if arg == 1 {
                    value.clone()
                } else {
                    Fraction::of_int_int(arg, 2).unwrap()
                };
                for op in 0..4 {
                    emit(format!("arithmetic:{i}:{arg}:{kind}:{op}"), || {
                        let output = match (kind, op) {
                            (0, 0) => Fraction::add_int_with_outer(value.clone(), arg)?,
                            (0, 1) => Fraction::subtract_int_with_outer(value.clone(), arg)?,
                            (0, 2) => Fraction::multiply_int_with_outer(value.clone(), arg)?,
                            (0, _) => Fraction::divide_int_with_outer(value.clone(), arg)?,
                            (1, 0) => Fraction::add_long_with_outer(value.clone(), i64::from(arg))?,
                            (1, 1) => {
                                Fraction::subtract_long_with_outer(value.clone(), i64::from(arg))?
                            }
                            (1, 2) => {
                                Fraction::multiply_long_with_outer(value.clone(), i64::from(arg))?
                            }
                            (1, _) => {
                                Fraction::divide_long_with_outer(value.clone(), i64::from(arg))?
                            }
                            (2, 0) => Fraction::add_big_integer_with_outer(
                                value.clone(),
                                BigInteger::value_of(i64::from(arg)),
                            )?,
                            (2, 1) => Fraction::subtract_big_integer_with_outer(
                                value.clone(),
                                BigInteger::value_of(i64::from(arg)),
                            )?,
                            (2, 2) => Fraction::multiply_big_integer_with_outer(
                                value.clone(),
                                BigInteger::value_of(i64::from(arg)),
                            )?,
                            (2, _) => Fraction::divide_big_integer_with_outer(
                                value.clone(),
                                BigInteger::value_of(i64::from(arg)),
                            )?,
                            (_, 0) => {
                                Fraction::add_big_fraction_with_outer(value.clone(), other.clone())?
                            }
                            (_, 1) => Fraction::subtract_big_fraction_with_outer(
                                value.clone(),
                                other.clone(),
                            )?,
                            (_, 2) => Fraction::multiply_big_fraction_with_outer(
                                value.clone(),
                                other.clone(),
                            )?,
                            _ => Fraction::divide_big_fraction_with_outer(
                                value.clone(),
                                other.clone(),
                            )?,
                        };
                        result(&value, output)
                    });
                }
            }
        }
        let others = [
            value.clone(),
            Fraction::of_big_integer_big_integer(n.clone(), d.clone()).unwrap(),
            Fraction::__static_ZERO(),
            Fraction::__static_ONE(),
            Fraction::of_int_int(-2, 3).unwrap(),
        ];
        for (j, other) in others.into_iter().enumerate() {
            emit(format!("compare:{i}:{j}"), || {
                Ok(value.compare_to(other.clone())?.to_string())
            });
            emit(format!("equals:{i}:{j}"), || {
                Ok(
                    Fraction::equals_with_outer(value.clone(), Some(Box::new(other.clone())))?
                        .to_string(),
                )
            });
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
        emit(format!("decimal-exact:{i}"), || {
            decimal(value.big_decimal_value()?)
        });
        for (m, mode) in [
            RoundingMode::UP,
            RoundingMode::DOWN,
            RoundingMode::CEILING,
            RoundingMode::FLOOR,
            RoundingMode::HALF_UP,
            RoundingMode::HALF_DOWN,
            RoundingMode::HALF_EVEN,
            RoundingMode::UNNECESSARY,
        ]
        .into_iter()
        .enumerate()
        {
            emit(format!("decimal-round:{i}:{m}"), || {
                decimal(value.big_decimal_value_rounding_mode(mode)?)
            });
            for scale in [-2, 0, 3] {
                emit(format!("decimal-scale:{i}:{m}:{scale}"), || {
                    decimal(value.big_decimal_value_int_rounding_mode(scale, mode)?)
                });
            }
        }
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
        1e20,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
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
    }
    for (i, text) in [
        "0",
        "1",
        "-2 / 3",
        " 4 / -6 ",
        "123456789012345678901234567890 / 7",
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
        Fraction::__static_ZERO(),
        Fraction::__static_ONE(),
        Fraction::of_int(-1),
    ]
    .into_iter()
    .enumerate()
    {
        emit(format!("minimum-exponent:{i}"), || {
            result(&value, Fraction::pow_with_outer(value.clone(), i32::MIN)?)
        });
    }
}
