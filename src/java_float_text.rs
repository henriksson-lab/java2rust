// Java numeric grammar accepts only ASCII after Java's <= U+0020 trim.
// A non-ASCII sentinel is therefore sufficient for grammar/error selection;
// the original units remain authoritative for Java's observable error text.
fn java_float_text_parse<T>(
    value: impl JavaTextView,
    parse: impl FnOnce(&str) -> Result<T, String>,
) -> Result<T, JavaException> {
    let raw = value.java_text_view();
    let (begin, end) = raw.java_trim_range();
    let trimmed = raw
        .substring(begin, end)
        .expect("validated numeric trim range");
    let projection: String = trimmed
        .units()
        .iter()
        .map(|unit| {
            if *unit <= 0x7f {
                *unit as u8 as char
            } else {
                '\u{fffd}'
            }
        })
        .collect();
    parse(&projection).map_err(|error| {
        if !trimmed.is_empty() && error == java_float_error(&projection) {
            let mut message = JavaUtf16::from_scalar("For input string: \"");
            message.push_text(&trimmed);
            message.push_text(&JavaUtf16::from_scalar("\""));
            JavaException::new("NumberFormatException", JavaString::from_payload(message))
        } else {
            JavaException::new("NumberFormatException", error)
        }
    })
}

pub fn java_float_parse_text_f64(value: impl JavaTextView) -> Result<f64, JavaException> {
    java_float_text_parse(value, |text| java_float_parse_f64(text))
}

pub fn java_float_parse_text_f32(value: impl JavaTextView) -> Result<f32, JavaException> {
    java_float_text_parse(value, |text| java_float_parse_f32(text))
}
