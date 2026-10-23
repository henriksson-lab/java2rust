// Shared by Java literal parsing and the emitted, dependency-free runtime.
// Decimal conversion delegates to Rust only after validating Java's grammar.
// Hex conversion rounds retained bits once, including subnormal results.

enum JavaFloatToken<'a> {
    Nan,
    Infinity(bool),
    Decimal(&'a str),
    Hex(&'a str),
}

fn java_float_error(value: &str) -> String {
    if value.is_empty() {
        "empty String".to_string()
    } else {
        format!("For input string: \"{value}\"")
    }
}

fn java_float_token(value: &str) -> Result<JavaFloatToken<'_>, String> {
    let value = value.trim_matches(|ch: char| ch <= '\u{20}');
    let unsigned = value.strip_prefix(['+', '-']).unwrap_or(value);
    if unsigned == "NaN" {
        return Ok(JavaFloatToken::Nan);
    }
    if unsigned == "Infinity" {
        return Ok(JavaFloatToken::Infinity(value.starts_with('-')));
    }
    let numeric = value.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(value);
    let unsigned = numeric.strip_prefix(['+', '-']).unwrap_or(numeric);
    if unsigned.starts_with("0x") || unsigned.starts_with("0X") {
        return Ok(JavaFloatToken::Hex(numeric));
    }
    let bytes = unsigned.as_bytes();
    let mut index = 0;
    let mut digits = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
        digits += 1;
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
            digits += 1;
        }
        if bytes.get(index) == Some(&b'.') {
            return Err("multiple points".to_string());
        }
    }
    if digits == 0 {
        return Err(java_float_error(value));
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == start {
            return Err(java_float_error(value));
        }
    }
    if index != bytes.len() {
        return Err(java_float_error(value));
    }
    Ok(JavaFloatToken::Decimal(numeric))
}

fn java_float_round_bits(top: u64, retained: i64, keep: i64, sticky: bool) -> u64 {
    if keep < 0 {
        return 0;
    }
    let shift = retained - keep;
    if shift <= 0 {
        return top << (-shift as u32);
    }
    let result = if shift == 64 { 0 } else { top >> shift };
    let guard = top & (1u64 << (shift - 1)) != 0;
    let lower = top & ((1u64 << (shift - 1)) - 1) != 0 || sticky;
    result + u64::from(guard && (lower || result & 1 != 0))
}

fn java_hex_float_bits(value: &str, precision: i64, exponent_bits: u32) -> Option<u64> {
    let negative = value.starts_with('-');
    let unsigned = value.strip_prefix(['+', '-']).unwrap_or(value);
    let hex = unsigned
        .strip_prefix("0x")
        .or_else(|| unsigned.strip_prefix("0X"))?;
    let (mantissa, exponent) = hex.split_once(['p', 'P'])?;
    let exponent_negative = exponent.starts_with('-');
    let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
    if exponent.is_empty() || !exponent.bytes().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let mut power = 0i64;
    for digit in exponent.bytes() {
        power = power
            .saturating_mul(10)
            .saturating_add((digit - b'0') as i64);
    }
    if exponent_negative {
        power = -power;
    }
    let mut point = false;
    let mut digits = 0;
    let mut fraction_digits = 0i64;
    let mut bit_length = 0i64;
    let mut retained = 0i64;
    let mut top = 0u64;
    let mut sticky = false;
    for ch in mantissa.bytes() {
        if ch == b'.' && !point {
            point = true;
            continue;
        }
        let digit = match ch {
            b'0'..=b'9' => ch - b'0',
            b'a'..=b'f' => ch - b'a' + 10,
            b'A'..=b'F' => ch - b'A' + 10,
            _ => return None,
        };
        digits += 1;
        if point {
            fraction_digits += 1;
        }
        for position in (0..4).rev() {
            let bit = (digit >> position) & 1;
            if bit_length == 0 && bit == 0 {
                continue;
            }
            bit_length += 1;
            if retained < 64 {
                top = (top << 1) | bit as u64;
                retained += 1;
            } else {
                sticky |= bit != 0;
            }
        }
    }
    if digits == 0 {
        return None;
    }
    let fraction_bits = precision - 1;
    let sign = u64::from(negative) << (fraction_bits + exponent_bits as i64);
    if bit_length == 0 {
        return Some(sign);
    }
    let bias = (1i64 << (exponent_bits - 1)) - 1;
    let minimum = 1 - bias;
    let mut exponent = power
        .saturating_add(bit_length - 1)
        .saturating_sub(4 * fraction_digits);
    if exponent < minimum {
        let keep = exponent.saturating_sub(minimum).saturating_add(precision);
        return Some(sign | java_float_round_bits(top, retained, keep, sticky));
    }
    let mut rounded = java_float_round_bits(top, retained, precision, sticky);
    if rounded == 1u64 << precision {
        rounded >>= 1;
        exponent = exponent.saturating_add(1);
    }
    if exponent > bias {
        return Some(sign | (((1u64 << exponent_bits) - 1) << fraction_bits));
    }
    Some(
        sign | (((exponent + bias) as u64) << fraction_bits)
            | (rounded & ((1u64 << fraction_bits) - 1)),
    )
}

pub fn java_float_parse_f64<V: AsRef<str>>(value: V) -> Result<f64, String> {
    let value = value.as_ref().trim_matches(|ch: char| ch <= '\u{20}');
    match java_float_token(value)? {
        JavaFloatToken::Nan => Ok(f64::NAN),
        JavaFloatToken::Infinity(negative) => Ok(if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        }),
        JavaFloatToken::Decimal(numeric) => numeric.parse().map_err(|_| java_float_error(value)),
        JavaFloatToken::Hex(numeric) => java_hex_float_bits(numeric, 53, 11)
            .map(f64::from_bits)
            .ok_or_else(|| java_float_error(value)),
    }
}

pub fn java_float_parse_f32<V: AsRef<str>>(value: V) -> Result<f32, String> {
    let value = value.as_ref().trim_matches(|ch: char| ch <= '\u{20}');
    match java_float_token(value)? {
        JavaFloatToken::Nan => Ok(f32::NAN),
        JavaFloatToken::Infinity(negative) => Ok(if negative {
            f32::NEG_INFINITY
        } else {
            f32::INFINITY
        }),
        JavaFloatToken::Decimal(numeric) => numeric.parse().map_err(|_| java_float_error(value)),
        JavaFloatToken::Hex(numeric) => java_hex_float_bits(numeric, 24, 8)
            .map(|bits| f32::from_bits(bits as u32))
            .ok_or_else(|| java_float_error(value)),
    }
}
