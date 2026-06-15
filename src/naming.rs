//! Port of `de.aschoerk.java2rust.utils.NamingHelper`.

/// Convert a camel-case string to its snake-case representation.
///
/// Acronym-aware: runs of capitals stay together (`JSONObject` →
/// `json_object`, `getJSONObject` → `get_json_object`, `parseURL` →
/// `parse_url`), splitting only at a lowercase/digit→upper boundary or where an
/// acronym meets the next word (an upper followed by a lower). Inspired by
/// `NamingHelper.camelToSnakeCase`, but the original split before every capital.
pub fn camel_to_snake_case(input: &str) -> String {
    if input.is_empty() {
        return input.to_string();
    }
    let chars: Vec<char> = input.chars().collect();
    let mut snake = String::new();
    for i in 0..chars.len() {
        let c = chars[i];
        if c.is_uppercase() {
            let prev = if i > 0 { Some(chars[i - 1]) } else { None };
            let next = chars.get(i + 1).copied();
            let boundary = match (prev, next) {
                // first character: never a boundary
                (None, _) => false,
                // a lowercase/digit before an upper always starts a new word
                (Some(p), _) if p.is_lowercase() || p.is_ascii_digit() => true,
                // upper preceded by upper but followed by lower: acronym → word
                // (`JSONObject`: split before the `O` of `Object`)
                (Some(p), Some(n)) if p.is_uppercase() && n.is_lowercase() => true,
                _ => false,
            };
            if boundary {
                snake.push('_');
            }
            for lc in c.to_lowercase() {
                snake.push(lc);
            }
        } else {
            snake.push(c);
        }
    }
    snake
}

#[cfg(test)]
mod tests {
    use super::camel_to_snake_case;

    fn assert_eq_snake(expected: &str, actual: &str) {
        assert_eq!(expected, camel_to_snake_case(actual));
    }

    #[test]
    fn test_camel_to_snake() {
        assert_eq_snake("hello_world", "HelloWorld");
        assert_eq_snake("hello_world", "helloWorld");
        assert_eq_snake(
            "my_awesome_stub_resolver_facade_ejb",
            "MyAwesomeStubResolverFacadeEjb",
        );
        assert_eq_snake("a_very_long_variable_name", "aVeryLongVariableName");
        assert_eq_snake("", "");
        assert_eq_snake("nothing", "nothing");
        assert_eq_snake("snake", "Snake");
    }

    #[test]
    fn test_acronyms_stay_together() {
        assert_eq_snake("json_object", "JSONObject");
        assert_eq_snake("get_json_object", "getJSONObject");
        assert_eq_snake("opt_json_array", "optJSONArray");
        assert_eq_snake("parse_url", "parseURL");
        assert_eq_snake("get_id", "getID");
        assert_eq_snake("sam_record", "SAMRecord");
        assert_eq_snake("io_exception", "IOException");
        assert_eq_snake("https_connection", "HTTPSConnection");
    }
}
