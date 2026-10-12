use new_java2rust_focused_output::*;

fn format(
    delimiter: &str,
    null_string: Option<&str>,
) -> Result<std::sync::Arc<std::sync::Mutex<CSVFormat>>, JavaException> {
    Ok(std::sync::Arc::new(std::sync::Mutex::new(
        CSVFormat::new_string_character_quote_mode_character_character_boolean_boolean_string_string_object_array_string_array_boolean_boolean_boolean_boolean_boolean_boolean_duplicate_header_mode(
            delimiter.to_string(),
            Some('"'),
            Some(QuoteMode::MINIMAL),
            None,
            None,
            false,
            true,
            Some("\r\n".to_string()),
            null_string.map(str::to_string),
            None,
            None,
            false,
            false,
            false,
            false,
            false,
            false,
            DuplicateHeaderMode::ALLOW_ALL,
        )?,
    )))
}

fn auto_header_comment_format() -> Result<std::sync::Arc<std::sync::Mutex<CSVFormat>>, JavaException>
{
    Ok(std::sync::Arc::new(std::sync::Mutex::new(
        CSVFormat::new_string_character_quote_mode_character_character_boolean_boolean_string_string_object_array_string_array_boolean_boolean_boolean_boolean_boolean_boolean_duplicate_header_mode(
            ",".to_string(),
            Some('"'),
            Some(QuoteMode::MINIMAL),
            Some('#'),
            None,
            false,
            true,
            Some("\r\n".to_string()),
            None,
            None,
            Some(&[] as &[Option<String>]),
            false,
            false,
            false,
            false,
            false,
            false,
            DuplicateHeaderMode::ALLOW_ALL,
        )?,
    )))
}

fn parse_one(
    input: &str,
    format: std::sync::Arc<std::sync::Mutex<CSVFormat>>,
) -> Result<Vec<Option<String>>, JavaException> {
    let parser = CSVParser::parse_string_c_s_v_format(input.to_string(), format)?;
    Ok(CSVParser::next_record_with_outer(parser)?.borrow().values())
}

fn visible(value: Option<&str>) -> String {
    match value {
        None => "<null>".to_string(),
        Some(value) => value
            .replace('\\', "\\\\")
            .replace('\r', "\\r")
            .replace('\n', "\\n")
            .replace('|', "\\|"),
    }
}

fn values(values: &[Option<String>]) -> String {
    values
        .iter()
        .map(|value| visible(value.as_deref()))
        .collect::<Vec<_>>()
        .join("|")
}

fn run() -> Result<(), JavaException> {
    let basic_format = format(",", None)?;
    let out = StringWriter::new();
    let mut printer = CSVPrinter::new(out.clone().into(), basic_format.clone())?;
    printer.print_record_object_array_varargs(vec![
        Some(Box::new("a".to_string()) as Box<dyn std::any::Any>),
        Some(Box::new("b,c".to_string()) as Box<dyn std::any::Any>),
    ])?;
    let rendered = out.to_string();
    println!("print={}", visible(Some(&rendered)));
    println!("roundtrip={}", values(&parse_one(&rendered, basic_format)?));
    println!(
        "quoted={}",
        values(&parse_one(
            "\"a\"\"b\",\"line1\r\nline2\",,\r\n",
            format(",", None)?,
        )?)
    );
    println!(
        "delimiter={}",
        values(&parse_one("left;\"x;y\"\r\n", format(";", None)?)?)
    );
    println!(
        "nulls={}",
        values(&parse_one(
            "NULL,\"NULL\",,\r\n",
            format(",", Some("NULL"))?
        )?)
    );

    let parser = CSVParser::parse_string_c_s_v_format(
        "# generated\r\nname,age\r\nAda,42\r\n".to_string(),
        auto_header_comment_format()?,
    )?;
    println!(
        "headers={}",
        CSVParser::get_header_names_with_outer(parser.clone()).join("|")
    );
    println!(
        "comment={}",
        visible(CSVParser::get_header_comment_with_outer(parser.clone()).as_deref())
    );
    let record = CSVParser::next_record_with_outer(parser)?;
    println!(
        "named={}|{}",
        visible(Some(&CSVRecord::get_string_with_outer(
            record.clone(),
            Some("name".to_string()),
        )?)),
        visible(Some(&CSVRecord::get_string_with_outer(
            record,
            Some("age".to_string()),
        )?)),
    );

    let parser =
        CSVParser::parse_string_c_s_v_format("\"unclosed".to_string(), format(",", None)?)?;
    match CSVParser::next_record_with_outer(parser) {
        Err(error) if error.is_instance(&["IOException"]) => {
            println!("malformed=IOException")
        }
        Err(error) => panic!("unexpected malformed-input error: {error:?}"),
        Ok(_) => println!("malformed=accepted"),
    }
    Ok(())
}

fn main() {
    run().unwrap();
}
