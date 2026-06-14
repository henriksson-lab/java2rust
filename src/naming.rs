//! Port of `de.aschoerk.java2rust.utils.NamingHelper`.

/// Convert a camel-case string to its snake-case representation.
///
/// Mirrors `NamingHelper.camelToSnakeCase`.
pub fn camel_to_snake_case(input: &str) -> String {
    // early return
    if input.is_empty() {
        return input.to_string();
    }

    let mut snake = String::new();
    let mut chars = input.chars();

    // lowercase first character
    let first = chars.next().unwrap();
    for lc in first.to_lowercase() {
        snake.push(lc);
    }

    // create snake case string
    for c in chars {
        if c.is_uppercase() {
            snake.push('_');
        }
        for lc in c.to_lowercase() {
            snake.push(lc);
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
}
