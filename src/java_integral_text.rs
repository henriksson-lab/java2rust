// The identity runtime supplies JavaTextView, JavaUtf16, JavaString,
// JavaException and the shared BMP Character.digit decimal leaf.
pub fn java_integral_parse_text<T: JavaTextView>(
    value: Option<T>,
    radix: i32,
    width: u32,
) -> Result<i64, JavaException> {
    let Some(value) = value else {
        return Err(JavaException::new(
            "NumberFormatException",
            "Cannot parse null string",
        ));
    };
    if radix < 2 {
        return Err(JavaException::new(
            "NumberFormatException",
            format!("radix {radix} less than Character.MIN_RADIX"),
        ));
    }
    if radix > 36 {
        return Err(JavaException::new(
            "NumberFormatException",
            format!("radix {radix} greater than Character.MAX_RADIX"),
        ));
    }
    let text = value.java_text_view();
    let units = text.units();
    let input_error = || java_integral_input_error(&text, radix, false);
    let (negative, digits) = match units.first() {
        Some(&45) => (true, &units[1..]),
        Some(&43) => (false, &units[1..]),
        _ => (false, units),
    };
    if digits.is_empty() {
        return Err(input_error());
    }
    // Byte/Short first use Integer.parseInt, then check the narrower range.
    let parse_width = if width == 64 { 64 } else { 32 };
    let limit = (1u64 << (parse_width - 1)) - u64::from(!negative);
    let mut magnitude = 0u64;
    for &unit in digits {
        let digit = java_numeric_decimal_digit(unit)
            .map(u32::from)
            .or_else(|| match unit {
                0x41..=0x5a => Some((unit - 0x41) as u32 + 10),
                0x61..=0x7a => Some((unit - 0x61) as u32 + 10),
                0xff21..=0xff3a => Some((unit - 0xff21) as u32 + 10),
                0xff41..=0xff5a => Some((unit - 0xff41) as u32 + 10),
                _ => None,
            })
            .filter(|digit| *digit < radix as u32)
            .ok_or_else(input_error)?;
        magnitude = magnitude
            .checked_mul(radix as u64)
            .and_then(|value| value.checked_add(digit as u64))
            .filter(|value| *value <= limit)
            .ok_or_else(input_error)?;
    }
    let signed = if negative {
        0u64.wrapping_sub(magnitude) as i64
    } else {
        magnitude as i64
    };
    if (width == 8 && !(-128..=127).contains(&signed))
        || (width == 16 && !(-32768..=32767).contains(&signed))
    {
        return Err(java_integral_input_error(&text, radix, true));
    }
    Ok(signed)
}

fn java_integral_input_error(text: &JavaUtf16, radix: i32, narrow_range: bool) -> JavaException {
    let prefix = if narrow_range {
        "Value out of range. Value:\""
    } else {
        "For input string: \""
    };
    let mut units = prefix.encode_utf16().collect::<Vec<_>>();
    units.extend_from_slice(text.units());
    units.push(34);
    if narrow_range {
        units.extend(format!(" Radix:{radix}").encode_utf16());
    } else if radix != 10 {
        units.extend(format!(" under radix {radix}").encode_utf16());
    }
    JavaException::new(
        "NumberFormatException",
        JavaString::from_payload(JavaUtf16::from_units(units)),
    )
}
