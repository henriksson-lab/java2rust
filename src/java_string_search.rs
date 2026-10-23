// Shared by the active source runtime and the lossless runtime export.
// JavaTextView keeps offsets and individual surrogate needles in code units.
pub fn java_string_length<V: JavaTextView>(value: V) -> i32 {
    value.java_text_length()
}

pub fn java_string_contains<V: JavaTextView, N: JavaStringSearchNeedle>(
    value: V,
    needle: N,
) -> bool {
    let value = value.java_text_view();
    needle
        .java_string_search_needle()
        .is_some_and(|needle| value.index_of(&needle, 0) >= 0)
}

pub fn java_string_starts_with<V: JavaTextView, N: JavaStringSearchNeedle>(
    value: V,
    needle: N,
) -> bool {
    let value = value.java_text_view();
    needle
        .java_string_search_needle()
        .is_some_and(|needle| value.units().starts_with(needle.units()))
}

pub fn java_string_ends_with<V: JavaTextView, N: JavaStringSearchNeedle>(
    value: V,
    needle: N,
) -> bool {
    let value = value.java_text_view();
    needle
        .java_string_search_needle()
        .is_some_and(|needle| value.units().ends_with(needle.units()))
}

pub fn java_string_index_of<V: JavaTextView, N: JavaStringSearchNeedle>(
    value: V,
    needle: N,
) -> i32 {
    java_string_index_of_from(value, needle, 0)
}

pub fn java_string_index_of_from<V: JavaTextView, N: JavaStringSearchNeedle>(
    value: V,
    needle: N,
    from_index: i32,
) -> i32 {
    let Some(needle) = needle.java_string_search_needle() else {
        return -1;
    };
    value.java_text_view().index_of(&needle, from_index)
}

pub fn java_string_last_index_of<V: JavaTextView, N: JavaStringSearchNeedle>(
    value: V,
    needle: N,
) -> i32 {
    java_string_last_index_of_from(value, needle, i32::MAX)
}

pub fn java_string_last_index_of_from<V: JavaTextView, N: JavaStringSearchNeedle>(
    value: V,
    needle: N,
    from_index: i32,
) -> i32 {
    let Some(needle) = needle.java_string_search_needle() else {
        return -1;
    };
    value.java_text_view().last_index_of(&needle, from_index)
}
