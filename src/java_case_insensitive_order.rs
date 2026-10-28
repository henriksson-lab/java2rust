// String.CASE_INSENSITIVE_ORDER is a pure native comparator. Decode valid
// pairs but preserve isolated surrogate units, which compare as their values.
pub fn java_case_insensitive_string_cmp<L: JavaTextView, R: JavaTextView>(
    left: &L,
    right: &R,
) -> std::cmp::Ordering {
    fn fold(point: u32) -> u32 {
        let index = JAVA_SIMPLE_CASE_FOLD.partition_point(|(_, end, _)| *end < point);
        match JAVA_SIMPLE_CASE_FOLD.get(index) {
            Some(&(start, _, delta)) if start <= point => (point as i32 + delta) as u32,
            _ => point,
        }
    }
    let left = left.java_text_view();
    let right = right.java_text_view();
    let points = |unit: Result<char, std::char::DecodeUtf16Error>| match unit {
        Ok(value) => fold(value as u32),
        Err(value) => value.unpaired_surrogate() as u32,
    };
    std::char::decode_utf16(left.units().iter().copied())
        .map(points)
        .cmp(std::char::decode_utf16(right.units().iter().copied()).map(points))
}
