//! Lossless literal-token decoding. Unicode translation precedes lexical
//! escapes (JLS 3.3, 3.10.7); all intermediate values are UTF-16 code units.
//! This handles tokens supplied by the parser, not Unicode escapes outside
//! literals that change the source's tokenization.

use crate::java_ir::Literal;

pub(crate) fn decode(src: &str) -> Result<Literal, &'static str> {
    let token = unicode_units(src.trim())?;
    let quote = token.first().copied().ok_or("empty Java literal")?;
    if token.last() != Some(&quote) || !matches!(quote, 34 | 39) || token.len() < 2 {
        return Err("invalid Java literal delimiters");
    }
    let text_block = quote == 34 && token.starts_with(&[34; 3]);
    let content = if text_block {
        if token.len() < 6 || !token.ends_with(&[34; 3]) {
            return Err("invalid Java text block delimiters");
        }
        text_block_content(&token[3..token.len() - 3])?
    } else {
        token[1..token.len() - 1].to_vec()
    };
    let units = lexical_units(&content, quote, text_block)?;
    if quote == 39 {
        if units.len() != 1 {
            return Err("Java char literal must contain exactly one UTF-16 code unit");
        }
        Ok(Literal::from_java_char_unit(units[0]))
    } else {
        Ok(Literal::from_java_string_units(units))
    }
}

fn unicode_units(src: &str) -> Result<Vec<u16>, &'static str> {
    let raw: Vec<_> = src.encode_utf16().collect();
    let mut out = Vec::with_capacity(raw.len());
    visit_unicode_units(&raw, |unit, _, _| out.push(unit))?;
    Ok(out)
}

/// tree-sitter-java accepts one `u`, whereas Java accepts any positive number.
/// Canonicalize only the spelling, not the translated character: decoding a
/// backslash before parsing and decoding it again in a literal would be wrong.
/// The eligibility scan is shared with value decoding, including escapes that
/// themselves produce backslashes. Unaffected sources borrow their input.
pub(crate) fn normalize_unicode_escape_spelling(
    src: &str,
) -> Result<NormalizedUnicode<'_>, &'static str> {
    if !src.contains("\\uu") {
        return Ok(NormalizedUnicode {
            source: std::borrow::Cow::Borrowed(src),
            removed_at_units: Vec::new(),
        });
    }
    let raw: Vec<_> = src.encode_utf16().collect();
    let mut normalized = Vec::with_capacity(raw.len());
    let mut removed_at_units = Vec::new();
    visit_unicode_units(&raw, |_, spelling, escaped| {
        if escaped && spelling.len() > 6 {
            removed_at_units.push((normalized.len() + 2, spelling.len() - 6));
            normalized.extend_from_slice(&[92, 117]);
            normalized.extend_from_slice(&spelling[spelling.len() - 4..]);
        } else {
            normalized.extend_from_slice(spelling);
        }
    })?;
    if !removed_at_units.is_empty() {
        // Only redundant ASCII 'u' characters were removed, so UTF-8 remains
        // lossless. No arbitrary Java UTF-16 payload is converted here.
        Ok(NormalizedUnicode {
            source: std::borrow::Cow::Owned(
                String::from_utf16(&normalized)
                    .expect("Unicode escape spelling preserves scalar source"),
            ),
            removed_at_units,
        })
    } else {
        Ok(NormalizedUnicode {
            source: std::borrow::Cow::Borrowed(src),
            removed_at_units,
        })
    }
}

pub(crate) struct NormalizedUnicode<'a> {
    pub source: std::borrow::Cow<'a, str>,
    removed_at_units: Vec<(usize, usize)>,
}

impl NormalizedUnicode<'_> {
    /// Equivalent spellings have the same parse structure. Restore byte/point
    /// ranges before extracting nodes with tree-sitter's range-edit API. This
    /// keeps diagnostics and nested declarations in original source coordinates
    /// without a second IR walk. Work is linear in source + escape count.
    pub fn restore_tree_ranges(&self, tree: &mut tree_sitter::Tree) {
        if self.removed_at_units.is_empty() {
            return;
        }
        let mut pending = self.removed_at_units.iter().peekable();
        let mut units = 0;
        let mut point = tree_sitter::Point::new(0, 0);
        let mut edits = Vec::with_capacity(self.removed_at_units.len());
        for (byte, ch) in self.source.char_indices() {
            if let Some(&&(offset, count)) = pending.peek()
                && offset == units
            {
                edits.push(tree_sitter::InputEdit {
                    start_byte: byte,
                    old_end_byte: byte,
                    new_end_byte: byte + count,
                    start_position: point,
                    old_end_position: point,
                    new_end_position: tree_sitter::Point::new(point.row, point.column + count),
                });
                pending.next();
            }
            units += ch.len_utf16();
            if ch == '\n' {
                point.row += 1;
                point.column = 0;
            } else {
                point.column += ch.len_utf8();
            }
        }
        assert!(
            pending.next().is_none(),
            "Unicode spelling edit must be on a scalar boundary"
        );
        for edit in edits.iter().rev() {
            tree.edit(edit);
        }
    }
}

fn visit_unicode_units(
    raw: &[u16],
    mut emit: impl FnMut(u16, &[u16], bool),
) -> Result<(), &'static str> {
    let mut i = 0;
    let mut slashes = 0;
    let mut last_from_escape = false;
    while i < raw.len() {
        let start = i;
        let mut unit = raw[i];
        let eligible = last_from_escape || slashes % 2 == 0;
        last_from_escape = false;
        i += 1;
        if unit == 92 && eligible && raw.get(i) == Some(&117) {
            while raw.get(i) == Some(&117) {
                i += 1;
            }
            unit = 0;
            for _ in 0..4 {
                let digit = match raw.get(i).copied() {
                    Some(ch @ 48..=57) => ch - 48,
                    Some(ch @ 65..=70) => ch - 65 + 10,
                    Some(ch @ 97..=102) => ch - 97 + 10,
                    _ => return Err("invalid Java Unicode escape"),
                };
                unit = (unit << 4) | digit;
                i += 1;
            }
            last_from_escape = true;
        }
        slashes = if unit == 92 { slashes + 1 } else { 0 };
        emit(unit, &raw[start..i], last_from_escape);
    }
    Ok(())
}

fn whitespace(unit: u16) -> bool {
    matches!(unit, 9 | 12 | 32)
}

fn text_block_content(raw: &[u16]) -> Result<Vec<u16>, &'static str> {
    // Normalize CR/CRLF before incidental indentation and lexical escapes.
    let mut normalized = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        let unit = raw[i];
        i += 1;
        if unit == 13 {
            if raw.get(i) == Some(&10) {
                i += 1;
            }
            normalized.push(10);
        } else {
            normalized.push(unit);
        }
    }
    let opening = normalized.iter().take_while(|&&u| whitespace(u)).count();
    if normalized.get(opening) != Some(&10) {
        return Err("Java text block opening delimiter requires a line terminator");
    }
    let lines: Vec<_> = normalized[opening + 1..].split(|&u| u == 10).collect();
    let indent = lines
        .iter()
        .enumerate()
        .filter(|(i, line)| *i + 1 == lines.len() || !line.iter().all(|&u| whitespace(u)))
        .map(|(_, line)| line.iter().take_while(|&&u| whitespace(u)).count())
        .min()
        .unwrap_or(0);
    let mut content = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if i != 0 {
            content.push(10);
        }
        let start = line
            .iter()
            .take(indent)
            .take_while(|&&u| whitespace(u))
            .count();
        let mut end = line.len();
        while end > start && whitespace(line[end - 1]) {
            end -= 1;
        }
        content.extend_from_slice(&line[start..end]);
    }
    Ok(content)
}

fn lexical_units(raw: &[u16], quote: u16, text_block: bool) -> Result<Vec<u16>, &'static str> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        let unit = raw[i];
        i += 1;
        if unit != 92 {
            if !text_block && (unit == quote || matches!(unit, 10 | 13)) {
                return Err("unescaped delimiter or line terminator in Java literal");
            }
            out.push(unit);
            continue;
        }
        let escaped = *raw.get(i).ok_or("unterminated Java escape")?;
        i += 1;
        let value = match escaped {
            98 => 8,   // b
            116 => 9,  // t
            110 => 10, // n
            102 => 12, // f
            114 => 13, // r
            115 => 32, // s
            34 | 39 | 92 => escaped,
            10 if text_block => continue,
            first @ 48..=55 => {
                let mut value = first - 48;
                let limit = if first <= 51 { 3 } else { 2 };
                for _ in 1..limit {
                    let Some(&digit @ 48..=55) = raw.get(i) else {
                        break;
                    };
                    value = value * 8 + digit - 48;
                    i += 1;
                }
                value
            }
            _ => return Err("invalid Java lexical escape"),
        };
        out.push(value);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string(token: &str) -> Vec<u16> {
        match decode(token).unwrap() {
            Literal::String(s) => s.encode_utf16().collect(),
            Literal::StringUtf16(units) => units,
            _ => panic!("expected String"),
        }
    }

    #[test]
    fn source_literals_preserve_code_units_and_normalize_only_valid_pairs() {
        assert_eq!(
            string(r#""A\u00e9\uD83D\uDE00Z""#),
            "Aé😀Z".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(string(r#""\uD83Dx\uDE00""#), [0xd83d, 120, 0xde00]);
        for unit in 0..=u16::MAX {
            let token = format!("'\\u{unit:04x}'");
            // Unicode translation produces raw lexical delimiters, backslash,
            // and line terminators; those need lexical escapes instead.
            if matches!(unit, 10 | 13 | 39 | 92) {
                continue;
            }
            assert_eq!(decode(&token).unwrap().java_char_unit(), Some(unit));
        }
    }

    #[test]
    fn unicode_translation_is_nonrecursive_and_obeys_backslash_eligibility() {
        assert_eq!(
            string(r#""\\u0041""#),
            r"\u0041".encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(string(r#""\\\u0041""#), [92, 65]);
        assert_eq!(string(r#""\uuuu0041""#), [65]);
        assert_eq!(string(r#""\u005cn""#), [10]);
        assert_eq!(string(r#""\u005c\u005cn""#), [92, 110]);
        assert!(decode(r#""\u005cu0041""#).is_err());
        assert!(decode(r#""\u000a""#).is_err());
        assert!(decode(r#""\u0022""#).is_err());
    }

    #[test]
    fn equivalent_unicode_spelling_normalization_does_not_translate_twice() {
        for (source, expected) in [
            (r#""\uuuu0041""#, r#""\u0041""#),
            (r#""\\uuuu0041""#, r#""\\uuuu0041""#),
            (r#""\\\uuuu0041""#, r#""\\\u0041""#),
            (r#""\u005c\uuuu005cn""#, r#""\u005c\u005cn""#),
            (r#""\uuuu005cu0041""#, r#""\u005cu0041""#),
        ] {
            assert_eq!(
                normalize_unicode_escape_spelling(source).unwrap().source,
                expected
            );
            assert_eq!(decode(source), decode(expected));
        }
    }

    #[test]
    fn lexical_escapes_share_one_decoder_and_octal_limits() {
        assert_eq!(
            string(r#""\b\t\n\f\r\s\"\'\\""#),
            [8, 9, 10, 12, 13, 32, 34, 39, 92]
        );
        assert_eq!(
            string(r#""\377\400\777\378""#),
            [255, 32, 48, 63, 55, 31, 56]
        );
        assert_eq!(decode(r"'\377'").unwrap().java_char_unit(), Some(255));
        assert_eq!(decode(r"'\b'").unwrap().java_char_unit(), Some(8));
        assert!(decode(r"'\400'").is_err());
        assert!(decode("'😀'").is_err());
        assert!(decode(r#""\8""#).is_err());
    }

    #[test]
    fn text_blocks_strip_indent_before_escapes_without_losing_surrogates() {
        assert_eq!(
            string("\"\"\" \t\r\n  A\\s  \r\n  \\uD83D\\\r\n  Z\r\n  \"\"\""),
            [65, 32, 10, 0xd83d, 90, 10]
        );
    }
}
