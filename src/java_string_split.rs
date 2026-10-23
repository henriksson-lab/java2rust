// Native String.split fast path. The planner must prove that the regex is a
// nonempty literal without surrogate units. Retain the actual argument and
// normal call staging: this kernel does not fold away argument evaluation.
pub fn java_string_split_literal_identity<R: JavaTextView>(
    value: &JavaString,
    pattern: R,
    limit: i32,
) -> Vec<Option<JavaString>> {
    let pattern = pattern.java_text_view();
    let delimiter = pattern.units();
    debug_assert!(!delimiter.is_empty());
    let units = value.units();
    let mut parts = Vec::new();
    let mut begin = 0;
    let mut search = 0;
    while search + delimiter.len() <= units.len()
        && (limit <= 0 || parts.len() + 1 < limit as usize)
    {
        if &units[search..search + delimiter.len()] == delimiter {
            parts.push(Some(
                value
                    .substring(begin as i32, search as i32)
                    .expect("validated literal split range"),
            ));
            search += delimiter.len();
            begin = search;
        } else {
            search += 1;
        }
    }
    // Java preserves the entire receiver, including empty input, if no
    // delimiter was consumed. Trailing-empty removal only follows a match.
    if parts.is_empty() {
        return vec![Some(value.clone())];
    }
    parts.push(Some(
        value
            .substring(begin as i32, value.len())
            .expect("validated literal split tail"),
    ));
    if limit == 0 {
        while parts
            .last()
            .is_some_and(|part| part.as_ref().is_some_and(JavaString::is_empty))
        {
            parts.pop();
        }
    }
    parts
}
