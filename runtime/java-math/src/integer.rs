//! Integer operations used by rational arithmetic and exact binary conversion.
//! Keep the Java API boundary here; callers remain translated source algorithms.
use super::numeric_text::java_numeric_decimal_digit as decimal_digit;
use super::{ArithmeticException, BigInteger, NumberFormatException};
use num_bigint::{BigInt, Sign};
use num_integer::Integer;
use num_traits::ToPrimitive;
use std::borrow::Borrow;

impl BigInteger {
    /// The modeled Java String constructor. Backend FromStr remains available
    /// independently; this boundary retains Java UTF-16 validation/diagnostics.
    pub fn new(value: impl AsRef<[u16]>) -> Result<Self, NumberFormatException> {
        parse_decimal(value.as_ref())
    }

    /// Java's sign/magnitude constructor copies unsigned big-endian octets.
    /// The mapped argument boundary checks null before entering this method.
    pub fn from_sign_magnitude(
        signum: i32,
        magnitude: &[i8],
    ) -> Result<Self, NumberFormatException> {
        let error = |message: &str| NumberFormatException(Some(message.encode_utf16().collect()));
        let sign = match signum {
            -1 => Sign::Minus,
            0 => Sign::NoSign,
            1 => Sign::Plus,
            _ => return Err(error("Invalid signum value")),
        };
        if signum == 0 && magnitude.iter().any(|byte| *byte != 0) {
            return Err(error("signum-magnitude mismatch"));
        }
        let bytes = magnitude.iter().map(|byte| *byte as u8).collect::<Vec<_>>();
        Ok(Self::from_value(BigInt::from_bytes_be(sign, &bytes)))
    }
    pub fn int_value_exact(&self) -> Result<i32, ArithmeticException> {
        i32::try_from(self.0.payload())
            .map_err(|_| ArithmeticException("BigInteger out of int range"))
    }

    pub fn long_value_exact(&self) -> Result<i64, ArithmeticException> {
        i64::try_from(self.0.payload())
            .map_err(|_| ArithmeticException("BigInteger out of long range"))
    }

    pub fn double_value(&self) -> f64 {
        // The pinned backend retains discarded sticky bits before converting
        // to IEEE nearest-even, including values wider than a machine word.
        self.0
            .to_f64()
            .expect("BigInt to_f64 always yields a finite value or infinity")
    }

    pub fn signum(&self) -> i32 {
        match self.0.sign() {
            Sign::Minus => -1,
            Sign::NoSign => 0,
            Sign::Plus => 1,
        }
    }

    pub fn abs(&self) -> Self {
        if self.signum() < 0 {
            self.negate()
        } else {
            self.clone()
        }
    }

    pub fn negate(&self) -> Self {
        Self::from_value(-self.0.payload())
    }

    pub fn add(&self, other: impl Borrow<Self>) -> Result<Self, ArithmeticException> {
        Self::checked(self.0.payload() + other.borrow().0.payload())
    }

    pub fn subtract(&self, other: impl Borrow<Self>) -> Result<Self, ArithmeticException> {
        Self::checked(self.0.payload() - other.borrow().0.payload())
    }

    pub fn divide(&self, other: impl Borrow<Self>) -> Result<Self, ArithmeticException> {
        let other = other.borrow();
        if other.0.sign() == Sign::NoSign {
            return Err(ArithmeticException("BigInteger divide by zero"));
        }
        // BigInt division truncates toward zero, as Java BigInteger requires.
        Ok(Self::from_value(self.0.payload() / other.0.payload()))
    }

    pub fn gcd(&self, other: impl Borrow<Self>) -> Self {
        Self::from_value(self.0.gcd(&other.borrow().0))
    }

    pub fn bit_length(&self) -> i32 {
        // Java excludes the sign bit: for negative values measure the infinite
        // two's-complement complement, not the unsigned magnitude.
        let bits = if self.0.sign() == Sign::Minus {
            (-self.0.payload() - BigInt::from(1)).bits()
        } else {
            self.0.bits()
        };
        bits as i32
    }

    pub fn get_lowest_set_bit(&self) -> i32 {
        self.0.trailing_zeros().map_or(-1, |bits| bits as i32)
    }

    pub fn flip_bit(&self, index: i32) -> Result<Self, ArithmeticException> {
        if index < 0 {
            return Err(ArithmeticException("Negative bit address"));
        }
        if index as u64 >= Self::MAX_MAGNITUDE_BITS {
            return Err(ArithmeticException(
                "BigInteger would overflow supported range",
            ));
        }
        Self::checked(self.0.payload() ^ (BigInt::from(1) << index as u32))
    }

    pub fn hash_code(&self) -> i32 {
        let (_, words) = self.0.to_u32_digits();
        words
            .iter()
            .rev()
            .fold(0i32, |hash, word| {
                hash.wrapping_mul(31).wrapping_add(*word as i32)
            })
            .wrapping_mul(self.signum())
    }
}

#[cfg(test)]
mod identity_tests {
    use super::BigInteger;

    #[test]
    fn immutable_reference_clones_keep_identity_while_equal_constructions_are_distinct() {
        let left: BigInteger = "12345678901234567890".parse().unwrap();
        let equal: BigInteger = "12345678901234567890".parse().unwrap();
        assert_eq!(left, equal);
        assert!(left.java_identity_eq(&left.clone()));
        assert!(!left.java_identity_eq(&equal));
        assert!(left.java_identity_eq(&left.abs()));
        let zero = BigInteger::new(vec![48]).unwrap();
        assert!(!zero.java_identity_eq(&BigInteger::new(vec![48]).unwrap()));
        assert!(!zero.java_identity_eq(&BigInteger::value_of(0)));
        assert!(BigInteger::value_of(1).java_identity_eq(&BigInteger::value_of(1)));
    }
}

fn parse_decimal(value: &[u16]) -> Result<BigInteger, NumberFormatException> {
    let error = |message: &str| NumberFormatException(Some(message.encode_utf16().collect()));
    if value.is_empty() {
        return Err(error("Zero length BigInteger"));
    }
    let minus = value.iter().rposition(|unit| *unit == 45);
    let plus = value.iter().rposition(|unit| *unit == 43);
    if minus.is_some_and(|index| index != 0 || plus.is_some())
        || plus.is_some_and(|index| index != 0)
    {
        return Err(error("Illegal embedded sign character"));
    }
    let mut cursor = usize::from(minus.is_some() || plus.is_some());
    if cursor == value.len() {
        return Err(error("Zero length BigInteger"));
    }
    while cursor < value.len() && decimal_digit(value[cursor]) == Some(0) {
        cursor += 1;
    }
    if cursor == value.len() {
        return Ok(BigInteger::from_value(BigInt::from(0)));
    }
    let mut digits = Vec::with_capacity(value.len() - cursor);
    // JDK diagnostics identify the first invalid decimal group, not a lossy
    // rendering of the whole String. Grouping affects only error selection;
    // the backend parses the validated normalized digits in one operation.
    let first = (value.len() - cursor - 1) % 9 + 1;
    let mut width = first;
    while cursor < value.len() {
        let group = &value[cursor..cursor + width];
        for unit in group {
            let Some(digit) = decimal_digit(*unit) else {
                let mut message = "For input string: \"".encode_utf16().collect::<Vec<_>>();
                message.extend_from_slice(group);
                message.push(34);
                return Err(NumberFormatException(Some(message)));
            };
            digits.push(b'0' + digit);
        }
        cursor += width;
        width = 9;
    }
    let magnitude = BigInt::parse_bytes(&digits, 10).expect("validated decimal digits");
    Ok(BigInteger::from_value(if minus.is_some() {
        -magnitude
    } else {
        magnitude
    }))
}

macro_rules! unary_adapter {
    ($name:ident, $result:ty) => {
        pub fn $name(value: impl Borrow<BigInteger>) -> $result {
            value.borrow().$name()
        }
    };
}
unary_adapter!(int_value_exact, Result<i32, ArithmeticException>);
unary_adapter!(long_value_exact, Result<i64, ArithmeticException>);
unary_adapter!(double_value, f64);
unary_adapter!(signum, i32);
unary_adapter!(abs, BigInteger);
unary_adapter!(negate, BigInteger);
unary_adapter!(bit_length, i32);
unary_adapter!(get_lowest_set_bit, i32);
unary_adapter!(hash_code, i32);

macro_rules! binary_adapter {
    ($name:ident, $result:ty) => {
        pub fn $name(value: impl Borrow<BigInteger>, other: &BigInteger) -> $result {
            value.borrow().$name(other)
        }
    };
}
binary_adapter!(add, Result<BigInteger, ArithmeticException>);
binary_adapter!(subtract, Result<BigInteger, ArithmeticException>);
binary_adapter!(divide, Result<BigInteger, ArithmeticException>);
binary_adapter!(gcd, BigInteger);

pub fn flip_bit(
    value: impl Borrow<BigInteger>,
    index: i32,
) -> Result<BigInteger, ArithmeticException> {
    value.borrow().flip_bit(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integer(value: &str) -> BigInteger {
        value.parse().unwrap()
    }

    #[test]
    fn exact_number_conversions_check_signed_bounds() {
        for value in [i32::MIN, -1, 0, 1, i32::MAX] {
            assert_eq!(
                BigInteger::value_of(i64::from(value))
                    .int_value_exact()
                    .unwrap(),
                value
            );
        }
        for value in [i64::MIN, -1, 0, 1, i64::MAX] {
            assert_eq!(
                BigInteger::value_of(value).long_value_exact().unwrap(),
                value
            );
        }
        for value in ["2147483648", "-2147483649"] {
            assert_eq!(
                integer(value).int_value_exact().unwrap_err().to_string(),
                "BigInteger out of int range"
            );
        }
        for value in ["9223372036854775808", "-9223372036854775809"] {
            assert_eq!(
                integer(value).long_value_exact().unwrap_err().to_string(),
                "BigInteger out of long range"
            );
        }
    }

    #[test]
    fn double_conversion_preserves_integer_rounding_and_wide_sticky_bits() {
        for value in [
            0,
            1,
            -1,
            (1_i64 << 53) - 1,
            1_i64 << 53,
            (1_i64 << 53) + 1,
            (1_i64 << 53) + 3,
            i64::MIN,
            i64::MAX,
        ] {
            assert_eq!(
                BigInteger::value_of(value).double_value().to_bits(),
                (value as f64).to_bits()
            );
        }
        assert_eq!(
            integer("18446744073709553665").double_value().to_bits(),
            0x43f0000000000001
        );
        assert_eq!(
            integer("-18446744073709553665").double_value().to_bits(),
            0xc3f0000000000001
        );
    }

    #[test]
    fn sign_magnitude_copies_unsigned_octets_and_validates_zero_and_sign() {
        for sign in [-1, 0, 1] {
            for bytes in [&[][..], &[0][..], &[0, 0][..]] {
                assert_eq!(
                    BigInteger::from_sign_magnitude(sign, bytes).unwrap(),
                    integer("0")
                );
            }
        }
        let mut bytes = [0, -128, 1];
        let positive = BigInteger::from_sign_magnitude(1, &bytes).unwrap();
        assert_eq!(positive, integer("32769"));
        assert_eq!(
            BigInteger::from_sign_magnitude(-1, &bytes).unwrap(),
            integer("-32769")
        );
        bytes[1] = 0;
        assert_eq!(positive, integer("32769"));
        assert_eq!(
            BigInteger::from_sign_magnitude(1, &[-1; 16]).unwrap(),
            integer("340282366920938463463374607431768211455")
        );
        for sign in [i32::MIN, -2, 2, i32::MAX] {
            assert_eq!(
                BigInteger::from_sign_magnitude(sign, &[])
                    .unwrap_err()
                    .to_string(),
                "Invalid signum value"
            );
        }
        assert_eq!(
            BigInteger::from_sign_magnitude(0, &[0, 1])
                .unwrap_err()
                .to_string(),
            "signum-magnitude mismatch"
        );
    }

    #[test]
    fn java_decimal_constructor_shares_digits_and_retains_error_units() {
        let parse = |value: &str| BigInteger::new(value.encode_utf16().collect::<Vec<_>>());
        for (source, expected) in [
            ("+000123", "123"),
            ("-００１２３", "-123"),
            ("١٢٣", "123"),
            ("-000", "0"),
        ] {
            assert_eq!(parse(source).unwrap(), integer(expected));
        }
        for (source, expected) in [
            ("", "Zero length BigInteger"),
            ("+", "Zero length BigInteger"),
            ("1-2", "Illegal embedded sign character"),
            ("+-1", "Illegal embedded sign character"),
            ("123456789x", "For input string: \"23456789x\""),
        ] {
            assert_eq!(parse(source).unwrap_err().to_string(), expected);
        }
        let error = BigInteger::new([0xd800]).unwrap_err();
        let mut expected = "For input string: \"".encode_utf16().collect::<Vec<_>>();
        expected.extend([0xd800, 34]);
        assert_eq!(
            super::super::number_format_message_utf16(&error),
            Some(expected)
        );
    }

    #[test]
    fn arithmetic_retains_unbounded_values_and_java_quotients() {
        let huge = integer("340282366920938463463374607431768211456");
        let divisor = integer("18446744073709551616");
        assert_eq!(huge.divide(&divisor).unwrap(), divisor);
        assert_eq!(huge.add(&huge).unwrap().subtract(&huge).unwrap(), huge);
        assert_eq!(huge.gcd(&divisor.negate()), divisor);
        assert_eq!(integer("-7").divide(integer("3")).unwrap(), integer("-2"));
        assert_eq!(integer("7").divide(integer("-3")).unwrap(), integer("-2"));
        assert_eq!(integer("0").gcd(integer("0")), integer("0"));
        assert_eq!(
            huge.divide(integer("0")).unwrap_err().to_string(),
            "BigInteger divide by zero"
        );
        assert_eq!(huge.signum(), 1);
        assert_eq!(huge.negate().signum(), -1);
        assert_eq!(huge.negate().abs(), huge);
    }

    #[test]
    fn twos_complement_bit_contracts_and_magnitude_hash() {
        for (value, length, lowest) in [
            ("0", 0, -1),
            ("1", 1, 0),
            ("-1", 0, 0),
            ("2", 2, 1),
            ("-2", 1, 1),
            ("-3", 2, 0),
            ("-8", 3, 3),
        ] {
            let value = integer(value);
            assert_eq!(value.bit_length(), length);
            assert_eq!(value.get_lowest_set_bit(), lowest);
        }
        assert_eq!(integer("0").flip_bit(128).unwrap().bit_length(), 129);
        assert_eq!(
            integer("-1").flip_bit(65).unwrap(),
            integer("-36893488147419103233")
        );
        assert_eq!(
            integer("-1").flip_bit(65).unwrap().flip_bit(65).unwrap(),
            integer("-1")
        );
        assert_eq!(
            integer("0").flip_bit(-1).unwrap_err().to_string(),
            "Negative bit address"
        );
        assert_eq!(integer("4294967296").hash_code(), 31);
        assert_eq!(integer("-4294967296").hash_code(), -31);
        assert_eq!(integer("18446744073709551615").hash_code(), -32);
    }
}
