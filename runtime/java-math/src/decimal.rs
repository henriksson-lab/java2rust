//! Exact coefficient/scale operations. Binary floating point enters only at
//! explicit constructors/conversions, never in decimal rounding or addition.
use super::{ArithmeticException, BigDecimal, BigInteger, NumberFormatException, RoundingMode};
use num_bigint::{BigInt, Sign};
use std::cmp::Ordering;

fn invalid(message: impl Into<String>) -> NumberFormatException {
    NumberFormatException(Some(message.into().encode_utf16().collect()))
}

// Character.digit(char, 10): BigDecimal's String parser consumes UTF-16 chars,
// so supplementary decimal digits are not accepted as a surrogate pair.
fn digit(character: u16) -> Option<u8> {
    const ZEROS: &[u32] = &[
        0x0030, 0x0660, 0x06f0, 0x07c0, 0x0966, 0x09e6, 0x0a66, 0x0ae6, 0x0b66, 0x0be6, 0x0c66,
        0x0ce6, 0x0d66, 0x0de6, 0x0e50, 0x0ed0, 0x0f20, 0x1040, 0x1090, 0x17e0, 0x1810, 0x1946,
        0x19d0, 0x1a80, 0x1a90, 0x1b50, 0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900, 0xa9d0,
        0xa9f0, 0xaa50, 0xabf0, 0xff10,
    ];
    let code = character as u32;
    ZEROS.iter().find_map(|zero| {
        code.checked_sub(*zero)
            .filter(|value| *value < 10)
            .map(|value| value as u8)
    })
}

pub(super) fn parse(value: &str) -> Result<BigDecimal, NumberFormatException> {
    parse_units(value.encode_utf16())
}

pub(super) fn parse_units(
    value: impl Iterator<Item = u16>,
) -> Result<BigDecimal, NumberFormatException> {
    let mut chars = value.peekable();
    if chars.peek().is_none() {
        return Err(NumberFormatException(None));
    }
    let negative = chars.peek() == Some(&45);
    if matches!(chars.peek(), Some(43 | 45)) {
        chars.next();
    }
    let mut digits = Vec::new();
    let mut fraction = 0i64;
    let mut point = false;
    while let Some(&character) = chars.peek() {
        if matches!(character, 101 | 69) {
            break;
        }
        chars.next();
        if character == 46 {
            if point {
                return Err(invalid(
                    "Character array contains more than one decimal point.",
                ));
            }
            point = true;
        } else if let Some(digit) = digit(character) {
            digits.push(b'0' + digit);
            if point {
                fraction += 1;
            }
        } else {
            let mut message = "Character ".encode_utf16().collect::<Vec<_>>();
            message.push(character);
            message.extend(" is neither a decimal digit number, decimal point, nor \"e\" notation exponential mark.".encode_utf16());
            return Err(NumberFormatException(Some(message)));
        }
    }
    if digits.is_empty() {
        return Err(invalid("No digits found."));
    }
    let mut exponent = 0i64;
    if chars.next().is_some() {
        let minus = chars.peek() == Some(&45);
        if matches!(chars.peek(), Some(43 | 45)) {
            chars.next();
        }
        if chars.peek().is_none() {
            return Err(NumberFormatException(None));
        }
        let mut significant_digits = 0;
        for character in chars {
            let digit = digit(character).ok_or_else(|| invalid("Not a digit."))?;
            if digit != 0 || significant_digits != 0 {
                significant_digits += 1;
            }
            if significant_digits > 10 {
                return Err(invalid("Too many nonzero exponent digits."));
            }
            exponent = exponent * 10 + i64::from(digit);
        }
        if minus {
            exponent = -exponent;
        }
    }
    let scale = i32::try_from(fraction - exponent).map_err(|_| invalid("Exponent overflow."))?;
    let mut coefficient = BigInt::parse_bytes(&digits, 10).expect("validated decimal digits");
    if negative {
        coefficient = -coefficient;
    }
    Ok(BigDecimal { coefficient, scale })
}

pub(super) fn from_double(value: f64) -> Result<BigDecimal, NumberFormatException> {
    if !value.is_finite() {
        return Err(invalid("Infinite or NaN"));
    }
    let bits = value.to_bits();
    let encoded_exponent = ((bits >> 52) & 0x7ff) as i32;
    let mut significand = bits & ((1u64 << 52) - 1);
    if encoded_exponent != 0 {
        significand |= 1u64 << 52;
    }
    if significand == 0 {
        return Ok(BigDecimal {
            coefficient: BigInt::from(0),
            scale: 0,
        });
    }
    let mut exponent = if encoded_exponent == 0 {
        -1074
    } else {
        encoded_exponent - 1023 - 52
    };
    if exponent < 0 {
        let cancel = significand.trailing_zeros().min((-exponent) as u32);
        significand >>= cancel;
        exponent += cancel as i32;
    }
    let (mut coefficient, scale) = if exponent >= 0 {
        (BigInt::from(significand) << exponent as usize, 0)
    } else {
        (
            BigInt::from(significand) * BigInt::from(5).pow((-exponent) as u32),
            -exponent,
        )
    };
    if bits >> 63 != 0 {
        coefficient = -coefficient;
    }
    Ok(BigDecimal { coefficient, scale })
}

fn ten_power(exponent: i64, coefficient_bits: u64) -> Result<BigInt, ArithmeticException> {
    let exponent = u32::try_from(exponent)
        .ok()
        .filter(|exponent| *exponent <= i32::MAX as u32)
        .ok_or(ArithmeticException("Underflow"))?;
    // A lower bound on log2(10) rejects provably out-of-range requests before
    // allocation. It is not an arbitrary application-specific scale limit.
    if u64::from(exponent) * 3_321_928 / 1_000_000 + coefficient_bits
        > BigInteger::MAX_MAGNITUDE_BITS
    {
        return Err(ArithmeticException(
            "BigInteger would overflow supported range",
        ));
    }
    Ok(BigInt::from(10).pow(exponent))
}

fn scale_up(value: &BigInt, amount: i64) -> Result<BigInt, ArithmeticException> {
    if value.sign() == Sign::NoSign {
        return Ok(value.clone());
    }
    let scaled = value * ten_power(amount, value.bits())?;
    if scaled.bits() > BigInteger::MAX_MAGNITUDE_BITS {
        return Err(ArithmeticException(
            "BigInteger would overflow supported range",
        ));
    }
    Ok(scaled)
}

pub(super) fn add(
    left: &BigDecimal,
    right: &BigDecimal,
) -> Result<BigDecimal, ArithmeticException> {
    let scale = left.scale.max(right.scale);
    let left = scale_up(&left.coefficient, i64::from(scale) - i64::from(left.scale))?;
    let right = scale_up(
        &right.coefficient,
        i64::from(scale) - i64::from(right.scale),
    )?;
    let coefficient = left + right;
    if coefficient.bits() > BigInteger::MAX_MAGNITUDE_BITS {
        return Err(ArithmeticException(
            "BigInteger would overflow supported range",
        ));
    }
    Ok(BigDecimal { coefficient, scale })
}

pub(super) fn set_scale(
    value: &BigDecimal,
    scale: i32,
    mode: RoundingMode,
) -> Result<BigDecimal, ArithmeticException> {
    if scale >= value.scale {
        return Ok(BigDecimal {
            coefficient: scale_up(
                &value.coefficient,
                i64::from(scale) - i64::from(value.scale),
            )?,
            scale,
        });
    }
    if value.coefficient.sign() == Sign::NoSign {
        return Ok(BigDecimal {
            coefficient: value.coefficient.clone(),
            scale,
        });
    }
    let divisor = ten_power(i64::from(value.scale) - i64::from(scale), 1)?;
    let mut quotient = &value.coefficient / &divisor;
    let remainder = &value.coefficient % &divisor;
    if remainder.sign() != Sign::NoSign {
        let half = (remainder.magnitude() << 1usize).cmp(divisor.magnitude());
        let increment = match mode {
            RoundingMode::UP => true,
            RoundingMode::DOWN => false,
            RoundingMode::CEILING => value.coefficient.sign() != Sign::Minus,
            RoundingMode::FLOOR => value.coefficient.sign() == Sign::Minus,
            RoundingMode::HALF_UP => half != Ordering::Less,
            RoundingMode::HALF_DOWN => half == Ordering::Greater,
            RoundingMode::HALF_EVEN => {
                half == Ordering::Greater || (half == Ordering::Equal && quotient.bit(0))
            }
            RoundingMode::UNNECESSARY => return Err(ArithmeticException("Rounding necessary")),
        };
        if increment {
            quotient += if value.coefficient.sign() == Sign::Minus {
                -1
            } else {
                1
            };
        }
    }
    Ok(BigDecimal {
        coefficient: quotient,
        scale,
    })
}

pub(super) fn format(value: &BigDecimal) -> String {
    let signed = value.coefficient.to_string();
    let digits = signed.strip_prefix('-').unwrap_or(&signed);
    let sign = if value.coefficient.sign() == Sign::Minus {
        "-"
    } else {
        ""
    };
    let adjusted = digits.len() as i64 - i64::from(value.scale) - 1;
    if value.scale >= 0 && adjusted >= -6 {
        if value.scale == 0 {
            return signed;
        }
        let point = digits.len() as i64 - i64::from(value.scale);
        if point > 0 {
            let point = point as usize;
            return format!("{sign}{}.{}", &digits[..point], &digits[point..]);
        }
        return format!("{sign}0.{}{digits}", "0".repeat((-point) as usize));
    }
    let fraction = if digits.len() > 1 {
        format!(".{}", &digits[1..])
    } else {
        String::new()
    };
    format!(
        "{sign}{}{fraction}E{}{adjusted}",
        &digits[..1],
        if adjusted >= 0 { "+" } else { "" }
    )
}

pub(super) fn double_value(value: &BigDecimal) -> f64 {
    format!("{}e{}", value.coefficient, -i64::from(value.scale))
        .parse()
        .expect("valid decimal conversion")
}
pub(super) fn float_value(value: &BigDecimal) -> f32 {
    format!("{}e{}", value.coefficient, -i64::from(value.scale))
        .parse()
        .expect("valid decimal conversion")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_binary_constructor_is_not_shortest_decimal_parsing() {
        let exact = from_double(0.1).unwrap();
        assert_eq!(
            format(&exact),
            "0.1000000000000000055511151231257827021181583404541015625"
        );
        assert_eq!(double_value(&exact).to_bits(), 0.1f64.to_bits());
        assert_ne!(exact, parse("0.1").unwrap());
        assert_eq!(format(&from_double(-0.0).unwrap()), "0");
        assert_eq!(
            from_double(f64::NAN).unwrap_err().to_string(),
            "Infinite or NaN"
        );
    }
    #[test]
    fn integer_rounding_retains_scale_and_obeys_tie_parity() {
        for (input, mode, expected) in [
            ("2.5", RoundingMode::HALF_EVEN, "2"),
            ("3.5", RoundingMode::HALF_EVEN, "4"),
            ("-2.5", RoundingMode::HALF_UP, "-3"),
            ("-2.5", RoundingMode::HALF_DOWN, "-2"),
            ("-1.1", RoundingMode::FLOOR, "-2"),
            ("-1.1", RoundingMode::CEILING, "-1"),
        ] {
            assert_eq!(
                format(&set_scale(&parse(input).unwrap(), 0, mode).unwrap()),
                expected
            );
        }
        assert_eq!(
            format(&add(&parse("1.00").unwrap(), &parse("2.0").unwrap()).unwrap()),
            "3.00"
        );
        assert_eq!(
            set_scale(&parse("1.1").unwrap(), 0, RoundingMode::UNNECESSARY)
                .unwrap_err()
                .to_string(),
            "Rounding necessary"
        );
    }
    #[test]
    fn zero_extreme_scales_and_unicode_decimal_digits_are_preserved() {
        assert_eq!(
            format(&set_scale(&parse("0").unwrap(), i32::MIN, RoundingMode::UP).unwrap()),
            "0E+2147483648"
        );
        assert_eq!(format(&parse("١٢.٣").unwrap()), "12.3");
        assert_eq!(format(&parse("1e2147483648").unwrap()), "1E+2147483648");
        assert_eq!(
            set_scale(&parse("1E+2147483647").unwrap(), 1, RoundingMode::DOWN)
                .unwrap_err()
                .to_string(),
            "Underflow"
        );
    }
}
