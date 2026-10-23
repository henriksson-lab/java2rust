pub fn java_string_substring(value: &JavaString, begin: i32, end: i32) -> JavaString {
    java_string_substring_result(value, begin, end)
        .unwrap_or_else(|ex| panic!("uncaught Java exception: {:?}", ex))
}

pub fn java_string_substring_result(
    value: &JavaString,
    begin: i32,
    end: i32,
) -> Result<JavaString, JavaException> {
    value.substring(begin, end).map_err(|_| {
        JavaException::new(
            "StringIndexOutOfBoundsException",
            "Java string index out of bounds".to_string(),
        )
    })
}
