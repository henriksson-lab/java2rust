//! Java numeric API adapters over an arbitrary-precision backend.
//!
//! Only operations declared in the accompanying external-pack manifest are
//! translation models. This is not a substitute implementation of caller code.
use num_bigint::{BigInt, ParseBigIntError, Sign};
use std::{borrow::Borrow, cmp::Ordering, fmt, str::FromStr};

mod decimal;
#[path = "../../../src/java_string_identity.rs"]
mod immutable_reference;
mod integer;
mod numeric_text;
/// OpenJDK's RoundingMode overloads dereference this internal enum field before
/// delegating to their integer-mode implementation, including unchanged scale.
pub fn rounding_mode_null_message() -> &'static str {
    "Cannot read field \"oldMode\" because \"roundingMode\" is null"
}
use immutable_reference::JavaImmutableReference;
pub use integer::{
    abs, add, bit_length, divide, double_value, flip_bit, gcd, get_lowest_set_bit, hash_code,
    int_value_exact, long_value_exact, negate, signum, subtract,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BigDecimal(JavaImmutableReference<BigDecimalValue>);

#[doc(hidden)]
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct BigDecimalValue {
    coefficient: BigInt,
    scale: i32,
}

impl std::ops::Deref for BigDecimal {
    type Target = BigDecimalValue;
    fn deref(&self) -> &BigDecimalValue {
        self.0.payload()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumberFormatException(Option<Vec<u16>>);

impl fmt::Display for NumberFormatException {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(&number_format_message(self).unwrap_or_default())
    }
}
impl std::error::Error for NumberFormatException {}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RoundingMode {
    UP,
    DOWN,
    CEILING,
    FLOOR,
    HALF_UP,
    HALF_DOWN,
    HALF_EVEN,
    UNNECESSARY,
}

impl RoundingMode {
    pub fn name(&self) -> String {
        format!("{self:?}")
    }
    pub fn ordinal(&self) -> i32 {
        *self as i32
    }
}
impl fmt::Display for RoundingMode {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "{self:?}")
    }
}

pub trait BigDecimalInput {
    fn into_big_decimal(self) -> Result<BigDecimal, NumberFormatException>;
}
impl BigDecimalInput for &str {
    fn into_big_decimal(self) -> Result<BigDecimal, NumberFormatException> {
        decimal::parse(self)
    }
}
impl BigDecimalInput for String {
    fn into_big_decimal(self) -> Result<BigDecimal, NumberFormatException> {
        decimal::parse(&self)
    }
}
impl BigDecimalInput for &String {
    fn into_big_decimal(self) -> Result<BigDecimal, NumberFormatException> {
        decimal::parse(self)
    }
}
impl BigDecimalInput for Vec<u16> {
    fn into_big_decimal(self) -> Result<BigDecimal, NumberFormatException> {
        decimal::parse_units(self.into_iter())
    }
}
impl BigDecimalInput for &[u16] {
    fn into_big_decimal(self) -> Result<BigDecimal, NumberFormatException> {
        decimal::parse_units(self.iter().copied())
    }
}
impl BigDecimalInput for f64 {
    fn into_big_decimal(self) -> Result<BigDecimal, NumberFormatException> {
        decimal::from_double(self)
    }
}
impl BigDecimalInput for f32 {
    fn into_big_decimal(self) -> Result<BigDecimal, NumberFormatException> {
        decimal::from_double(f64::from(self))
    }
}
impl BigDecimalInput for i32 {
    fn into_big_decimal(self) -> Result<BigDecimal, NumberFormatException> {
        Ok(BigDecimal::from_parts(BigInt::from(self), 0))
    }
}
impl BigDecimalInput for i64 {
    fn into_big_decimal(self) -> Result<BigDecimal, NumberFormatException> {
        Ok(BigDecimal::from_parts(BigInt::from(self), 0))
    }
}
impl BigDecimalInput for BigInteger {
    fn into_big_decimal(self) -> Result<BigDecimal, NumberFormatException> {
        Ok(BigDecimal::from_parts(self.0.payload().clone(), 0))
    }
}
impl BigDecimalInput for &BigInteger {
    fn into_big_decimal(self) -> Result<BigDecimal, NumberFormatException> {
        self.clone().into_big_decimal()
    }
}

impl BigDecimal {
    fn from_parts(coefficient: BigInt, scale: i32) -> Self {
        Self(JavaImmutableReference::from_payload(std::sync::Arc::new(
            BigDecimalValue { coefficient, scale },
        )))
    }
    pub fn java_identity_eq(&self, other: &Self) -> bool {
        self.0.java_identity_eq(&other.0)
    }

    pub fn new<T: BigDecimalInput>(value: T) -> Result<Self, NumberFormatException> {
        value.into_big_decimal()
    }
    pub fn add(&self, other: impl Borrow<Self>) -> Result<Self, ArithmeticException> {
        decimal::add(self, other.borrow())
    }
    pub fn subtract(&self, other: impl Borrow<Self>) -> Result<Self, ArithmeticException> {
        let other = other.borrow();
        decimal::add(self, &Self::from_parts(-&other.coefficient, other.scale))
    }
    pub fn set_scale(&self, scale: i32, mode: RoundingMode) -> Result<Self, ArithmeticException> {
        decimal::set_scale(self, scale, mode)
    }
    pub fn divide(&self, other: impl Borrow<Self>) -> Result<Self, ArithmeticException> {
        decimal::divide_exact(self, other.borrow())
    }
    pub fn divide_rounding(
        &self,
        other: impl Borrow<Self>,
        mode: RoundingMode,
    ) -> Result<Self, ArithmeticException> {
        decimal::divide_scaled(self, other.borrow(), self.scale, mode)
    }
    pub fn divide_scaled(
        &self,
        other: impl Borrow<Self>,
        scale: i32,
        mode: RoundingMode,
    ) -> Result<Self, ArithmeticException> {
        decimal::divide_scaled(self, other.borrow(), scale, mode)
    }
    pub fn double_value(&self) -> f64 {
        decimal::double_value(self)
    }
    pub fn float_value(&self) -> f32 {
        decimal::float_value(self)
    }
    pub fn to_big_integer(&self) -> Result<BigInteger, ArithmeticException> {
        decimal::to_big_integer(self)
    }
    pub fn scale(&self) -> i32 {
        self.scale
    }
}
impl fmt::Display for BigDecimal {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(&decimal::format(self))
    }
}

pub fn decimal_add(
    value: impl Borrow<BigDecimal>,
    other: &BigDecimal,
) -> Result<BigDecimal, ArithmeticException> {
    value.borrow().add(other)
}
pub fn decimal_set_scale(
    value: impl Borrow<BigDecimal>,
    scale: i32,
    mode: RoundingMode,
) -> Result<BigDecimal, ArithmeticException> {
    value.borrow().set_scale(scale, mode)
}
pub fn decimal_subtract(
    value: impl Borrow<BigDecimal>,
    other: &BigDecimal,
) -> Result<BigDecimal, ArithmeticException> {
    value.borrow().subtract(other)
}
pub fn decimal_divide(
    value: impl Borrow<BigDecimal>,
    other: &BigDecimal,
) -> Result<BigDecimal, ArithmeticException> {
    value.borrow().divide(other)
}
pub fn decimal_divide_rounding(
    value: impl Borrow<BigDecimal>,
    other: &BigDecimal,
    mode: RoundingMode,
) -> Result<BigDecimal, ArithmeticException> {
    value.borrow().divide_rounding(other, mode)
}
pub fn decimal_divide_scaled(
    value: impl Borrow<BigDecimal>,
    other: &BigDecimal,
    scale: i32,
    mode: RoundingMode,
) -> Result<BigDecimal, ArithmeticException> {
    value.borrow().divide_scaled(other, scale, mode)
}
pub fn decimal_to_big_integer(
    value: impl Borrow<BigDecimal>,
) -> Result<BigInteger, ArithmeticException> {
    value.borrow().to_big_integer()
}
pub fn decimal_double_value(value: impl Borrow<BigDecimal>) -> f64 {
    value.borrow().double_value()
}
pub fn decimal_float_value(value: impl Borrow<BigDecimal>) -> f32 {
    value.borrow().float_value()
}
pub fn decimal_scale(value: impl Borrow<BigDecimal>) -> i32 {
    value.borrow().scale()
}
pub fn decimal_to_string(value: impl Borrow<BigDecimal>) -> String {
    value.borrow().to_string()
}
pub fn rounding_mode_name(value: impl Borrow<RoundingMode>) -> String {
    value.borrow().name()
}
pub fn rounding_mode_ordinal(value: impl Borrow<RoundingMode>) -> i32 {
    value.borrow().ordinal()
}
pub fn number_format_message(value: impl Borrow<NumberFormatException>) -> Option<String> {
    // Native diagnostic/output adapter. Logical Java callers use the unit ABI
    // below; replacement belongs here, never in parsing or exception storage.
    value.borrow().0.as_ref().map(|units| {
        std::char::decode_utf16(units.iter().copied())
            .map(|scalar| scalar.unwrap_or('?'))
            .collect()
    })
}

pub fn decimal_to_string_utf16(value: impl Borrow<BigDecimal>) -> Vec<u16> {
    decimal_to_string(value).encode_utf16().collect()
}
pub fn rounding_mode_name_utf16(value: impl Borrow<RoundingMode>) -> Vec<u16> {
    rounding_mode_name(value).encode_utf16().collect()
}
pub fn number_format_message_utf16(value: impl Borrow<NumberFormatException>) -> Option<Vec<u16>> {
    value.borrow().0.clone()
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BigInteger(JavaImmutableReference<BigInt>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArithmeticException(&'static str);

impl fmt::Display for ArithmeticException {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(self.0)
    }
}
impl std::error::Error for ArithmeticException {}

impl fmt::Display for BigInteger {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.0.payload(), output)
    }
}

impl FromStr for BigInteger {
    type Err = ParseBigIntError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self::from_value)
    }
}

impl BigInteger {
    fn from_value(value: BigInt) -> Self {
        Self(JavaImmutableReference::from_payload(std::sync::Arc::new(
            value,
        )))
    }

    pub fn java_identity_eq(&self, other: &Self) -> bool {
        self.0.java_identity_eq(&other.0)
    }

    pub fn value_of(value: i64) -> Self {
        // JDK constants/valueOf retain a canonical small reference; String
        // constructors still allocate their own independent objects.
        static SMALL: std::sync::OnceLock<Vec<BigInteger>> = std::sync::OnceLock::new();
        if (-16..=16).contains(&value) {
            return SMALL.get_or_init(|| {
                (-16..=16)
                    .map(|value| Self::from_value(BigInt::from(value)))
                    .collect()
            })[(value + 16) as usize]
                .clone();
        }
        Self::from_value(BigInt::from(value))
    }

    // Match the reference JDK's supported magnitude rather than allowing an
    // extreme shift/exponent to wrap or attempt a provably invalid allocation.
    const MAX_MAGNITUDE_BITS: u64 = i32::MAX as u64;

    fn checked(value: BigInt) -> Result<Self, ArithmeticException> {
        if value.bits() > Self::MAX_MAGNITUDE_BITS {
            Err(ArithmeticException(
                "BigInteger would overflow supported range",
            ))
        } else {
            Ok(Self::from_value(value))
        }
    }

    pub fn multiply(&self, other: impl Borrow<Self>) -> Result<Self, ArithmeticException> {
        let other = other.borrow();
        if self.0.bits() != 0
            && other.0.bits() != 0
            && self.0.bits() + other.0.bits() - 1 > Self::MAX_MAGNITUDE_BITS
        {
            return Err(ArithmeticException(
                "BigInteger would overflow supported range",
            ));
        }
        Self::checked(self.0.payload() * other.0.payload())
    }

    pub fn pow(&self, exponent: i32) -> Result<Self, ArithmeticException> {
        if exponent < 0 {
            return Err(ArithmeticException("Negative exponent"));
        }
        if self.0.bits() != 0
            && (self.0.bits() - 1) * exponent as u64 + 1 > Self::MAX_MAGNITUDE_BITS
        {
            return Err(ArithmeticException(
                "BigInteger would overflow supported range",
            ));
        }
        Self::checked(self.0.pow(exponent as u32))
    }

    pub fn compare_to(&self, other: impl Borrow<Self>) -> i32 {
        match self.0.cmp(&other.borrow().0) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        }
    }

    pub fn test_bit(&self, index: i32) -> Result<bool, ArithmeticException> {
        if index < 0 {
            return Err(ArithmeticException("Negative bit address"));
        }
        Ok(self.0.bit(index as u64))
    }

    pub fn shift_right(&self, distance: i32) -> Result<Self, ArithmeticException> {
        self.shift_magnitude(distance.unsigned_abs(), distance < 0)
    }

    pub fn shift_left(&self, distance: i32) -> Result<Self, ArithmeticException> {
        self.shift_magnitude(distance.unsigned_abs(), distance >= 0)
    }

    fn shift_magnitude(&self, distance: u32, left: bool) -> Result<Self, ArithmeticException> {
        if distance == 0 {
            return Ok(self.clone());
        }
        if !left {
            return Ok(Self::from_value(self.0.payload() >> distance));
        }
        if self.0.bits() == 0 {
            return Ok(Self::value_of(0));
        }
        if self.0.bits() + u64::from(distance) > Self::MAX_MAGNITUDE_BITS {
            return Err(ArithmeticException(
                "BigInteger would overflow supported range",
            ));
        }
        Self::checked(self.0.payload() << distance)
    }

    pub fn mod_pow(
        &self,
        exponent: impl std::borrow::Borrow<Self>,
        modulus: impl std::borrow::Borrow<Self>,
    ) -> Result<Self, ArithmeticException> {
        let exponent = exponent.borrow();
        let modulus = modulus.borrow();
        if modulus.0.sign() != Sign::Plus {
            return Err(ArithmeticException("BigInteger: modulus not positive"));
        }
        // Java permits modulus one, including a negative exponent with a zero
        // base. Its sole residue is zero; do not attempt an inverse first.
        if modulus.0.payload() == &BigInt::from(1u8) {
            return Ok(Self::value_of(0));
        }
        if exponent.0.sign() == Sign::Minus {
            let inverse = self
                .0
                .modinv(&modulus.0)
                .ok_or(ArithmeticException("BigInteger not invertible."))?;
            Ok(Self::from_value(
                inverse.modpow(&(-exponent.0.payload()), &modulus.0),
            ))
        } else {
            Ok(Self::from_value(self.0.modpow(&exponent.0, &modulus.0)))
        }
    }

    pub fn int_value(&self) -> i32 {
        i32::from_le_bytes(self.low_signed_bytes())
    }

    pub fn long_value(&self) -> i64 {
        i64::from_le_bytes(self.low_signed_bytes())
    }

    fn low_signed_bytes<const N: usize>(&self) -> [u8; N] {
        // Java Number conversion truncates to the low two's-complement bits;
        // range-checked backend conversions would reject large values instead.
        let extension = if self.0.sign() == Sign::Minus {
            0xff
        } else {
            0
        };
        let mut output = [extension; N];
        for (destination, source) in output.iter_mut().zip(self.0.to_signed_bytes_le()) {
            *destination = source;
        }
        output
    }
}

// The existing external-pack receiver ABI passes the receiver as the first
// argument of a crate function. Accept owned temporaries and ordinary borrows
// without copying the underlying arbitrary-precision value.
pub fn mod_pow(
    value: impl std::borrow::Borrow<BigInteger>,
    exponent: &BigInteger,
    modulus: &BigInteger,
) -> Result<BigInteger, ArithmeticException> {
    value.borrow().mod_pow(exponent, modulus)
}

pub fn int_value(value: impl std::borrow::Borrow<BigInteger>) -> i32 {
    value.borrow().int_value()
}

pub fn long_value(value: impl std::borrow::Borrow<BigInteger>) -> i64 {
    value.borrow().long_value()
}

pub fn multiply(
    value: impl Borrow<BigInteger>,
    other: &BigInteger,
) -> Result<BigInteger, ArithmeticException> {
    value.borrow().multiply(other)
}

pub fn pow(
    value: impl Borrow<BigInteger>,
    exponent: i32,
) -> Result<BigInteger, ArithmeticException> {
    value.borrow().pow(exponent)
}

pub fn compare_to(value: impl Borrow<BigInteger>, other: &BigInteger) -> i32 {
    value.borrow().compare_to(other)
}

pub fn test_bit(value: impl Borrow<BigInteger>, index: i32) -> Result<bool, ArithmeticException> {
    value.borrow().test_bit(index)
}

pub fn shift_left(
    value: impl Borrow<BigInteger>,
    distance: i32,
) -> Result<BigInteger, ArithmeticException> {
    value.borrow().shift_left(distance)
}

pub fn shift_right(
    value: impl Borrow<BigInteger>,
    distance: i32,
) -> Result<BigInteger, ArithmeticException> {
    value.borrow().shift_right(distance)
}

pub fn to_string(value: impl Borrow<BigInteger>) -> String {
    value.borrow().to_string()
}
pub fn integer_to_string_utf16(value: impl Borrow<BigInteger>) -> Vec<u16> {
    to_string(value).encode_utf16().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integer(value: &str) -> BigInteger {
        value.parse().unwrap()
    }

    #[test]
    fn directional_shifts_share_sign_range_and_reference_contracts() {
        for text in ["0", "3", "-9", "18446744073709551617"] {
            let value = integer(text);
            assert!(value.java_identity_eq(&value.shift_left(0).unwrap()));
            assert!(value.java_identity_eq(&value.shift_right(0).unwrap()));
            for distance in [-65, -4, -1, 0, 1, 4, 65] {
                assert_eq!(
                    value.shift_left(distance).unwrap(),
                    value.shift_right(-distance).unwrap()
                );
            }
        }
        assert_eq!(integer("-9").shift_left(-1).unwrap(), integer("-5"));
        assert_eq!(
            integer("3").shift_left(64).unwrap(),
            integer("55340232221128654848")
        );
        assert_eq!(integer("-9").shift_left(i32::MIN).unwrap(), integer("-1"));
        assert_eq!(integer("0").shift_left(i32::MAX).unwrap(), integer("0"));
        assert_eq!(
            integer("2").shift_left(i32::MAX).unwrap_err().to_string(),
            "BigInteger would overflow supported range"
        );
    }

    #[test]
    fn multiplication_power_and_comparison_preserve_arbitrary_precision() {
        let value = integer("123456789012345678901234567890");
        assert_eq!(
            value.multiply(&value).unwrap().to_string(),
            "15241578753238836750495351562536198787501905199875019052100"
        );
        assert_eq!(integer("-3").pow(20).unwrap(), integer("3486784401"));
        assert_eq!(integer("-3").pow(7).unwrap(), integer("-2187"));
        assert_eq!(
            value.compare_to(integer("123456789012345678901234567889")),
            1
        );
        assert_eq!(value.compare_to(&value), 0);
        assert_eq!(
            integer("-1000000000000000000000000000").compare_to(integer("-1")),
            -1
        );
        assert_eq!(value.to_string(), "123456789012345678901234567890");
    }

    #[test]
    fn bit_and_shift_semantics_use_infinite_twos_complement() {
        assert_eq!(integer("-3").shift_right(1).unwrap(), integer("-2"));
        assert_eq!(
            integer("-3").shift_right(-65).unwrap().to_string(),
            "-110680464442257309696"
        );
        for value in ["-1", "-2", "-123456789012345678901234567890"] {
            assert!(integer(value).test_bit(i32::MAX).unwrap());
            assert_eq!(integer(value).shift_right(i32::MAX).unwrap(), integer("-1"));
        }
        assert!(!integer("-2").test_bit(0).unwrap());
        assert!(integer("-2").test_bit(1).unwrap());
        assert!(!integer("123456789012345678901234567890")
            .test_bit(i32::MAX)
            .unwrap());
        assert_eq!(
            integer("123456789012345678901234567890")
                .shift_right(i32::MAX)
                .unwrap(),
            integer("0")
        );
    }

    #[test]
    fn extreme_arguments_fail_before_invalid_allocations() {
        assert_eq!(
            integer("2").pow(i32::MAX).unwrap_err().to_string(),
            "BigInteger would overflow supported range"
        );
        assert_eq!(integer("1").pow(i32::MAX).unwrap(), integer("1"));
        assert_eq!(integer("-1").pow(i32::MAX).unwrap(), integer("-1"));
        assert_eq!(integer("0").pow(0).unwrap(), integer("1"));
        assert_eq!(integer("0").shift_right(i32::MIN).unwrap(), integer("0"));
        assert_eq!(
            integer("1").shift_right(i32::MIN).unwrap_err().to_string(),
            "BigInteger would overflow supported range"
        );
        assert_eq!(
            integer("0").pow(-1).unwrap_err().to_string(),
            "Negative exponent"
        );
        assert_eq!(
            integer("0").test_bit(-1).unwrap_err().to_string(),
            "Negative bit address"
        );
    }

    #[test]
    fn modular_power_is_arbitrary_precision() {
        // Independent JDK BigInteger observation, not a fixed-width surrogate.
        let result = integer("123456789012345678901234567890")
            .mod_pow(
                integer("1234567"),
                integer("987654321098765432109876543211"),
            )
            .unwrap();
        assert_eq!(result.to_string(), "855359452140779483336806875665");
    }

    #[test]
    fn modular_power_handles_signs_inverse_and_failures() {
        assert_eq!(
            integer("-2").mod_pow(integer("-1"), integer("5")).unwrap(),
            integer("2")
        );
        assert_eq!(
            integer("2").mod_pow(integer("-2"), integer("5")).unwrap(),
            integer("4")
        );
        assert_eq!(
            integer("-2").mod_pow(integer("3"), integer("5")).unwrap(),
            integer("2")
        );
        assert_eq!(
            integer("0").mod_pow(integer("-1"), integer("1")).unwrap(),
            integer("0")
        );
        assert_eq!(
            integer("0").mod_pow(integer("0"), integer("5")).unwrap(),
            integer("1")
        );
        for modulus in ["0", "-5"] {
            assert_eq!(
                integer("2")
                    .mod_pow(integer("0"), integer(modulus))
                    .unwrap_err()
                    .to_string(),
                "BigInteger: modulus not positive"
            );
        }
        for base in ["0", "2", "6"] {
            assert_eq!(
                integer(base)
                    .mod_pow(integer("-1"), integer("4"))
                    .unwrap_err()
                    .to_string(),
                "BigInteger not invertible."
            );
        }
    }

    #[test]
    fn number_conversions_truncate_and_sign_extend() {
        for (source, expected) in [
            ("0", 0),
            ("1", 1),
            ("-1", -1),
            ("2147483648", i32::MIN),
            ("4294967295", -1),
            ("4294967296", 0),
            ("-4294967297", -1),
            ("18446744073709551617", 1),
            ("-18446744073709551617", -1),
        ] {
            assert_eq!(integer(source).int_value(), expected, "{source}");
        }
        assert_eq!(integer("9223372036854775808").long_value(), i64::MIN);
        assert_eq!(integer("18446744073709551615").long_value(), -1);
        assert_eq!(integer("-18446744073709551617").long_value(), -1);
        for value in [i64::MIN, -1, 0, 1, i64::MAX] {
            assert_eq!(BigInteger::value_of(value).long_value(), value);
        }
    }
}
