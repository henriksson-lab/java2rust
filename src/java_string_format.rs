fn java_format_scientific(value: f64, precision: usize, uppercase: bool) -> String {
    if value.is_nan() {
        return if uppercase { "NAN" } else { "NaN" }.to_string();
    }
    if value.is_infinite() {
        let infinity = if uppercase { "INFINITY" } else { "Infinity" };
        return if value.is_sign_negative() {
            format!("-{infinity}")
        } else {
            infinity.to_string()
        };
    }
    let rendered = if uppercase {
        format!("{:.*E}", precision, value)
    } else {
        format!("{:.*e}", precision, value)
    };
    let Some((mantissa, exponent)) = rendered.rsplit_once(['e', 'E']) else {
        return rendered;
    };
    let exponent = exponent.parse::<i32>().unwrap_or(0);
    let marker = if uppercase { 'E' } else { 'e' };
    format!("{mantissa}{marker}{exponent:+03}")
}

fn java_format_argument(value: &str, directive: &str, conversion: char) -> String {
    let precision = directive.split_once('.').and_then(|(_, suffix)| {
        suffix
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>()
            .parse::<usize>()
            .ok()
    });
    match conversion {
        'f' | 'F' => value
            .parse::<f64>()
            .map(|number| {
                if number.is_nan() {
                    "NaN".to_string()
                } else if number.is_infinite() {
                    if number.is_sign_negative() {
                        "-Infinity".to_string()
                    } else {
                        "Infinity".to_string()
                    }
                } else {
                    format!("{:.*}", precision.unwrap_or(6), number)
                }
            })
            .unwrap_or_else(|_| value.to_string()),
        'e' => value
            .parse::<f64>()
            .map(|number| java_format_scientific(number, precision.unwrap_or(6), false))
            .unwrap_or_else(|_| value.to_string()),
        'E' => value
            .parse::<f64>()
            .map(|number| java_format_scientific(number, precision.unwrap_or(6), true))
            .unwrap_or_else(|_| value.to_string()),
        'x' => value
            .parse::<i128>()
            .map(|number| format!("{number:x}"))
            .unwrap_or_else(|_| value.to_string()),
        'X' => value
            .parse::<i128>()
            .map(|number| format!("{number:X}"))
            .unwrap_or_else(|_| value.to_string()),
        'o' => value
            .parse::<i128>()
            .map(|number| format!("{number:o}"))
            .unwrap_or_else(|_| value.to_string()),
        'S' => value.to_uppercase(),
        'B' => value.to_uppercase(),
        _ => value.to_string(),
    }
}

fn java_format_text_argument(value: &JavaUtf16, directive: &str, conversion: char) -> JavaUtf16 {
    if matches!(conversion, 'S' | 'B') {
        let mut output = Vec::new();
        for point in std::char::decode_utf16(value.units().iter().copied()) {
            match point {
                Ok(point) => {
                    let mapped = java_format_argument(&point.to_string(), directive, conversion);
                    output.extend(mapped.encode_utf16());
                }
                Err(error) => output.push(error.unpaired_surrogate()),
            }
        }
        return JavaUtf16::from_units(output);
    }
    if matches!(conversion, 'f' | 'F' | 'e' | 'E' | 'x' | 'X' | 'o') {
        // Numeric tokens are scalar text. Ordinary text directives never need
        // this projection, and malformed text is never replaced or truncated.
        if let Ok(scalar) = value.scalar_string() {
            return JavaUtf16::from_scalar(&java_format_argument(&scalar, directive, conversion));
        }
    }
    value.clone()
}

pub fn java_string_format<S: JavaTextView, A: JavaTextView>(
    template: S,
    args: Vec<A>,
) -> JavaFormatString {
    java_string_format_with_counted_arguments(template, Some(args.len()), |index| {
        Ok(args[index as usize].java_text_view().into_owned())
    })
    .unwrap_or_else(|error| std::panic::panic_any(error))
}

// Request an argument only at its directive; no array read loan crosses display.
pub fn java_string_format_with_arguments<S: JavaTextView, A: JavaTextView, E>(
    template: S,
    mut argument: impl FnMut(i32) -> Result<A, E>,
) -> Result<JavaFormatString, E> {
    java_string_format_kernel(template, |index, _directive| argument(index))
}

pub fn java_string_format_with_counted_arguments<S: JavaTextView, A: JavaTextView>(
    template: S,
    count: Option<usize>,
    mut argument: impl FnMut(i32) -> Result<A, JavaException>,
) -> Result<JavaFormatString, JavaException> {
    java_string_format_kernel(template, |index, directive| {
        if count.is_some_and(|count| index as usize >= count) {
            let mut message = JavaUtf16::from_scalar("Format specifier '%");
            message.push_text(&JavaUtf16::from_units(directive.to_vec()));
            message.push_text(&JavaUtf16::from_scalar("'"));
            return Err(JavaException::new(
                "MissingFormatArgumentException",
                java_format_finish_text(message),
            ));
        }
        argument(index)
    })
}

fn java_string_format_kernel<S: JavaTextView, A: JavaTextView, E>(
    template: S,
    mut argument: impl FnMut(i32, &[u16]) -> Result<A, E>,
) -> Result<JavaFormatString, E> {
    let template = template.java_text_view();
    let mut units = template.units().iter().copied().peekable();
    let mut out = Vec::with_capacity(template.units().len());
    let mut arg_index = 0i32;
    while let Some(unit) = units.next() {
        if unit != b'%' as u16 {
            out.push(unit);
            continue;
        }
        if units.peek() == Some(&(b'%' as u16)) {
            units.next();
            out.push(b'%' as u16);
            continue;
        }
        if units.peek() == Some(&(b'n' as u16)) {
            units.next();
            out.push(b'\n' as u16);
            continue;
        }
        let mut directive = Vec::new();
        while let Some(next) = units.next() {
            directive.push(next);
            if next <= 0x7f && (next as u8).is_ascii_alphabetic() {
                let conversion = next as u8 as char;
                if matches!(
                    conversion,
                    's' | 'S' | 'd' | 'b' | 'B' | 'c' | 'f' | 'F' | 'e' | 'E' | 'x' | 'X' | 'o'
                ) {
                    let value = argument(arg_index, &directive)?;
                    // Only ASCII belongs to the directive grammar. Keep a
                    // sentinel for non-scalar syntax instead of erasing it;
                    // literal spans and argument text retain their raw units.
                    let directive: String = directive
                        .iter()
                        .map(|unit| char::from_u32(*unit as u32).unwrap_or('\u{fffd}'))
                        .collect();
                    out.extend(
                        java_format_text_argument(&value.java_text_view(), &directive, conversion)
                            .units(),
                    );
                    arg_index += 1;
                } else {
                    out.push(b'%' as u16);
                    out.extend(&directive);
                }
                break;
            }
        }
    }
    Ok(java_format_finish_text(JavaUtf16::from_units(out)))
}
