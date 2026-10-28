use new_java2rust_focused_output::*;

fn format(
    delimiter: &str,
    null_string: Option<&str>,
) -> Result<std::sync::Arc<std::sync::Mutex<CSVFormat>>, JavaException> {
    format_options(delimiter, null_string, false)
}

fn format_options(
    delimiter: &str,
    null_string: Option<&str>,
    ignore_surrounding_spaces: bool,
) -> Result<std::sync::Arc<std::sync::Mutex<CSVFormat>>, JavaException> {
    format_details(delimiter, null_string, ignore_surrounding_spaces, None)
}

fn format_details(
    delimiter: &str,
    null_string: Option<&str>,
    ignore_surrounding_spaces: bool,
    escape: Option<JavaCodeUnit>,
) -> Result<std::sync::Arc<std::sync::Mutex<CSVFormat>>, JavaException> {
    Ok(std::sync::Arc::new(std::sync::Mutex::new(
        CSVFormat::new_string_character_quote_mode_character_character_boolean_boolean_string_string_object_array_string_array_boolean_boolean_boolean_boolean_boolean_boolean_duplicate_header_mode(
            JavaString::fresh(delimiter),
            Some(JavaCodeUnit::from_u16('"' as u16)),
            Some(QuoteMode::MINIMAL),
            None,
            escape,
            ignore_surrounding_spaces,
            true,
            Some(JavaString::intern_literal("\r\n")),
            null_string.map(JavaString::fresh),
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
    let header = java_register_sync_array_class(
        std::sync::Arc::new(std::sync::Mutex::new(Vec::<Option<JavaString>>::new())),
        "[Ljava.lang.String;",
    );
    Ok(std::sync::Arc::new(std::sync::Mutex::new(
        CSVFormat::new_string_character_quote_mode_character_character_boolean_boolean_string_string_object_array_string_array_boolean_boolean_boolean_boolean_boolean_boolean_duplicate_header_mode(
            JavaString::intern_literal(","),
            Some(JavaCodeUnit::from_u16('"' as u16)),
            Some(QuoteMode::MINIMAL),
            Some(JavaCodeUnit::from_u16('#' as u16)),
            None,
            false,
            true,
            Some(JavaString::intern_literal("\r\n")),
            None,
            None,
            Some(&header),
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

fn first_record(
    parser: std::rc::Rc<std::cell::RefCell<CSVParser>>,
) -> Result<std::rc::Rc<std::cell::RefCell<CSVRecord>>, JavaException> {
    CSVParser::iterator_with_outer(parser)
        .expect("parser iterator")
        .next_result()
}

fn parse_one(
    input: &str,
    format: std::sync::Arc<std::sync::Mutex<CSVFormat>>,
) -> Result<Vec<Option<JavaString>>, JavaException> {
    parse_one_text(JavaString::fresh(input), format)
}

fn parse_one_text(
    input: JavaString,
    format: std::sync::Arc<std::sync::Mutex<CSVFormat>>,
) -> Result<Vec<Option<JavaString>>, JavaException> {
    let parser = CSVParser::parse_string_c_s_v_format(input, format)?;
    let record = first_record(parser.clone());
    let close = CSVParser::close_with_outer(parser);
    let record = record?;
    close?;
    let array = record.borrow().values().expect("record values");
    let values = array.lock().unwrap().clone();
    Ok(values)
}

// Native decoding is confined to the transcript boundary. CSV retains UTF-16.
fn native(value: &JavaString) -> String {
    String::from_utf16_lossy(value.units())
}

fn visible_java(value: Option<&JavaString>) -> String {
    visible(value.map(native).as_deref())
}

fn object_array(
    values: Vec<Option<Box<dyn std::any::Any>>>,
) -> Result<Option<JavaReferenceArrayView>, JavaException> {
    let owner = java_register_local_array_class(
        std::rc::Rc::new(std::cell::RefCell::new(values)),
        "[Ljava.lang.Object;",
    );
    let erased = java_erase_local_object_array(owner);
    java_reference_array_cast_result(Some(erased.as_ref()))
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

fn values(values: &[Option<JavaString>]) -> String {
    values
        .iter()
        .map(|value| visible_java(value.as_ref()))
        .collect::<Vec<_>>()
        .join("|")
}

fn code_units(value: Option<&JavaString>) -> String {
    match value {
        None => "<null>".to_string(),
        Some(value) => value
            .units()
            .iter()
            .map(|unit| format!("{unit:04x}"))
            .collect::<Vec<_>>()
            .join("."),
    }
}

fn unit_values(values: &[Option<JavaString>]) -> String {
    values
        .iter()
        .map(|value| code_units(value.as_ref()))
        .collect::<Vec<_>>()
        .join("|")
}

fn escaped_stream() -> Result<JavaStream<std::rc::Rc<std::cell::RefCell<CSVRecord>>>, JavaException>
{
    let parser = CSVParser::parse_string_c_s_v_format(
        JavaString::intern_literal("escaped,1\r\nowner,2\r\n"),
        format(",", None)?,
    )?;
    CSVParser::stream_with_outer(parser)
}

fn run() -> Result<(), JavaException> {
    let basic_format = format(",", None)?;
    let out = StringWriter::new();
    let mut printer = CSVPrinter::new(out.clone().into(), basic_format.clone())?;
    printer.print_record_object_array_varargs(object_array(vec![
        Some(Box::new(JavaString::intern_literal("a")) as Box<dyn std::any::Any>),
        Some(Box::new(JavaString::intern_literal("b,c")) as Box<dyn std::any::Any>),
    ])?)?;
    printer.close()?;
    let rendered = out.to_string();
    println!("print={}", visible_java(Some(&rendered)));
    println!(
        "roundtrip={}",
        values(&parse_one(&native(&rendered), basic_format)?)
    );
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
        JavaString::intern_literal("# generated\r\nname,age\r\nAda,42\r\n"),
        auto_header_comment_format()?,
    )?;
    println!(
        "headers={}",
        CSVParser::get_header_names_with_outer(parser.clone())
            .borrow()
            .cloned_items()
            .iter()
            .map(|value| native(value.as_ref().expect("header name")))
            .collect::<Vec<_>>()
            .join("|")
    );
    println!(
        "comment={}",
        visible_java(CSVParser::get_header_comment_with_outer(parser.clone()).as_ref())
    );
    let record = first_record(parser.clone())?;
    println!(
        "named={}|{}",
        visible_java(
            CSVRecord::get_string_with_outer(
                record.clone(),
                Some(JavaString::intern_literal("name")),
            )?
            .as_ref()
        ),
        visible_java(
            CSVRecord::get_string_with_outer(record, Some(JavaString::intern_literal("age")),)?
                .as_ref()
        ),
    );

    CSVParser::close_with_outer(parser)?;

    let parser = CSVParser::parse_string_c_s_v_format(
        JavaString::intern_literal("\"unclosed"),
        format(",", None)?,
    )?;
    let result = first_record(parser.clone());
    CSVParser::close_with_outer(parser)?;
    match result {
        Err(error) if error.is_instance(&["IOException", "UncheckedIOException"]) => {
            println!("malformed=IOException")
        }
        Err(error) => panic!("unexpected malformed-input error: {error:?}"),
        Ok(_) => println!("malformed=accepted"),
    }
    let raw_input = JavaString::fresh(JavaUtf16::from_units(vec![
        0xd800, 0x002c, 0xdfff, 0x002c, 0xd83d, 0xde80, 0x000d, 0x000a,
    ]));
    println!(
        "utf16={}",
        unit_values(&parse_one_text(raw_input, format(",", None)?)?)
    );
    let raw_out = StringWriter::new();
    let mut raw_printer = CSVPrinter::new(raw_out.clone().into(), format(",", None)?)?;
    raw_printer.print_record_object_array_varargs(object_array(vec![Some(Box::new(
        JavaString::fresh(JavaUtf16::from_units(vec![0xd800, 0x002c, 0xdfff])),
    ))])?)?;
    raw_printer.close()?;
    println!("utf16-print={}", code_units(Some(&raw_out.to_string())));
    let spaced = "\u{001c}a\u{001c},\u{0085}b\u{0085},\u{00a0}c\u{00a0},\u{2003}d\u{2003},\u{2007}e\u{2007},\u{202f}f\u{202f}\r\n";
    println!(
        "whitespace={}",
        unit_values(&parse_one(spaced, format_options(",", None, true)?)?)
    );
    let parser = CSVParser::parse_string_c_s_v_format(
        JavaString::intern_literal("a,1\r\nb,2\r\nc,3\r\n"),
        format(",", None)?,
    )?;
    let stream = CSVParser::stream_with_outer(parser.clone())?;
    let before = CSVParser::get_record_number_with_outer(parser.clone());
    first_record(parser.clone())?;
    println!(
        "stream-alias={before}|{}|{}",
        stream.count_result()?,
        CSVParser::get_record_number_with_outer(parser.clone()),
    );
    match stream.count_result() {
        Err(error) if error.is_instance(&["IllegalStateException"]) => {
            println!("stream-reuse=IllegalStateException")
        }
        other => panic!("unexpected reused stream result: {other:?}"),
    }
    CSVParser::close_with_outer(parser)?;
    let stream = escaped_stream()?;
    println!("stream-escape={}", stream.count_result()?);
    stream.close();

    let parser = CSVParser::parse_string_c_s_v_format(
        JavaString::intern_literal("a\r\nb\r\n"),
        format(",", None)?,
    )?;
    let stream = CSVParser::stream_with_outer(parser.clone())?;
    stream.close();
    println!(
        "stream-close={}|{}",
        CSVParser::is_closed_with_outer(parser.clone())?,
        CSVParser::get_records_with_outer(parser.clone())?.len(),
    );
    match stream.count_result() {
        Err(error) if error.is_instance(&["IllegalStateException"]) => {
            println!("stream-closed-use=IllegalStateException")
        }
        other => panic!("unexpected closed stream result: {other:?}"),
    }
    CSVParser::close_with_outer(parser)?;

    let parser = CSVParser::parse_string_c_s_v_format(
        JavaString::intern_literal("a\r\nb\r\n"),
        format(",", None)?,
    )?;
    let stream = CSVParser::stream_with_outer(parser.clone())?;
    CSVParser::close_with_outer(parser)?;
    println!("parser-close={}", stream.count_result()?);

    let parser = CSVParser::parse_string_c_s_v_format(
        JavaString::intern_literal("a\r\nb\r\n"),
        format(",", None)?,
    )?;
    let matched = CSVParser::stream_with_outer(parser.clone())?.any_match_result(|_| {
        CSVParser::close_with_outer(parser.clone())?;
        Ok(true)
    })?;
    println!(
        "stream-reentry={matched}|{}|{}",
        CSVParser::get_record_number_with_outer(parser.clone()),
        CSVParser::is_closed_with_outer(parser.clone())?,
    );
    CSVParser::close_with_outer(parser)?;

    let parser = CSVParser::parse_string_c_s_v_format(
        JavaString::intern_literal("\"unclosed"),
        format(",", None)?,
    )?;
    let stream = CSVParser::stream_with_outer(parser.clone())?;
    let before = CSVParser::get_record_number_with_outer(parser.clone());
    match stream.count_result() {
        Err(error) if error.is_instance(&["UncheckedIOException"]) => {
            println!("stream-malformed={before}|UncheckedIOException")
        }
        Err(error) => panic!("unexpected malformed stream error: {error:?}"),
        Ok(_) => println!("stream-malformed=accepted"),
    }
    CSVParser::close_with_outer(parser)?;
    let parser = CSVParser::parse_string_c_s_v_format(
        JavaString::intern_literal("left,right\r\n"),
        format(",", None)?,
    )?;
    let record = first_record(parser.clone())?;
    let array = CSVRecord::values_with_outer(record.clone()).expect("record values");
    let snapshot = CSVRecord::to_list_with_outer(record.clone())?;
    let stream = CSVRecord::stream_with_outer(record.clone())?;
    array.lock().unwrap()[0] = Some(JavaString::intern_literal("changed"));
    array.lock().unwrap()[1] = None;
    println!(
        "array-alias={}|{}",
        std::sync::Arc::ptr_eq(
            &array,
            &CSVRecord::values_with_outer(record.clone()).unwrap()
        ),
        visible_java(CSVRecord::get_int_with_outer(record.clone(), 0)?.as_ref()),
    );
    let traversed = stream
        .into_iter_once_result()?
        .collect::<Result<Vec<_>, JavaException>>()?;
    println!("record-stream={}", values(&traversed));
    println!("list-copy={}", values(&snapshot));
    array.lock().unwrap()[0] = Some(JavaString::intern_literal("one"));
    array.lock().unwrap()[1] = Some(JavaString::intern_literal("two"));
    let mut seen = Vec::new();
    CSVRecord::stream_with_outer(record.clone())?.for_each_ordered_result(|value| {
        seen.push(value);
        if seen.len() == 1 {
            array.lock().unwrap()[1] = Some(JavaString::intern_literal("after"));
        }
        Ok(())
    })?;
    println!("array-reentry={}", values(&seen));
    let no_header = std::rc::Rc::new(std::cell::RefCell::new(JavaSemanticHashMap::<
        JavaString,
        Option<JavaString>,
    >::new()));
    no_header.borrow_mut().insert(
        JavaString::intern_literal("sentinel"),
        Some(JavaString::intern_literal("keep")),
    );
    let returned = CSVRecord::put_in_with_outer(record, no_header.clone())?;
    println!(
        "map-no-header={}|{}",
        std::rc::Rc::ptr_eq(&returned, &no_header),
        no_header.borrow().len()
    );
    CSVParser::close_with_outer(parser)?;

    let parser = CSVParser::parse_string_c_s_v_format(
        JavaString::intern_literal("a,b\r\nx,y\r\n"),
        auto_header_comment_format()?,
    )?;
    let record = first_record(parser.clone())?;
    CSVRecord::values_with_outer(record.clone())
        .unwrap()
        .lock()
        .unwrap()[1] = None;
    let map = std::rc::Rc::new(std::cell::RefCell::new(JavaSemanticHashMap::<
        JavaString,
        Option<JavaString>,
    >::new()));
    for (key, value) in [("sentinel", "keep"), ("a", "old"), ("b", "old")] {
        map.borrow_mut().insert(
            JavaString::intern_literal(key),
            Some(JavaString::intern_literal(value)),
        );
    }
    let returned = CSVRecord::put_in_with_outer(record.clone(), map.clone())?;
    returned.borrow_mut().insert(
        JavaString::intern_literal("later"),
        Some(JavaString::intern_literal("added")),
    );
    {
        let entries = map.borrow();
        let value = |key: &str| {
            visible_java(
                entries
                    .get(&JavaString::intern_literal(key))
                    .and_then(Option::as_ref),
            )
        };
        println!(
            "map-identity={}|{}|{}|{}|{}|{}|{}",
            std::rc::Rc::ptr_eq(&returned, &map),
            entries.len(),
            entries.contains_key(&JavaString::intern_literal("b")),
            value("a"),
            value("b"),
            value("sentinel"),
            value("later"),
        );
    }
    let mut copy = CSVRecord::to_map_with_outer(record.clone())?;
    copy.insert(
        JavaString::intern_literal("a"),
        Some(JavaString::intern_literal("copy")),
    );
    println!(
        "map-copy={}|{}|{}|{}",
        visible_java(
            CSVRecord::get_string_with_outer(record, Some(JavaString::intern_literal("a")))?
                .as_ref()
        ),
        visible_java(
            copy.get(&JavaString::intern_literal("a"))
                .and_then(Option::as_ref)
        ),
        copy.contains_key(&JavaString::intern_literal("b")),
        visible_java(
            copy.get(&JavaString::intern_literal("b"))
                .and_then(Option::as_ref)
        ),
    );
    CSVParser::close_with_outer(parser)?;
    let nested_out = StringWriter::new();
    let mut printer = CSVPrinter::new(nested_out.clone().into(), format(",", None)?)?;
    let row = java_register_sync_array_class(
        std::sync::Arc::new(std::sync::Mutex::new(vec![
            Some(JavaString::intern_literal("a")),
            Some(JavaString::intern_literal("b")),
        ])),
        "[Ljava.lang.String;",
    );
    let list = Iterable::from_reference_array(
        object_array(vec![Some(Box::new(JavaString::intern_literal("c"))), None])?.unwrap(),
    );
    printer.print_records_object_array_varargs(
        object_array(vec![
            Some(java_erase_shared_final_reference_array(row)),
            Some(list.erased_reference(|value| value)),
        ])?
        .unwrap(),
    )?;
    printer.close()?;
    println!(
        "nested-print={}",
        visible_java(Some(&nested_out.to_string()))
    );
    let stream_out = StringWriter::new();
    let mut printer = CSVPrinter::new(stream_out.clone().into(), format(",", None)?)?;
    printer.print_record_stream(JavaStream::from_iter(vec![
        Box::new(JavaString::intern_literal("s")) as Box<dyn std::any::Any>,
        Box::new(JavaString::intern_literal("t")) as Box<dyn std::any::Any>,
    ]))?;
    printer.close()?;
    println!(
        "stream-print={}",
        visible_java(Some(&stream_out.to_string()))
    );
    let mut reader = ExtendedBufferedReader::new(Some(
        StringReader::new(JavaString::intern_literal("a")).into(),
    ))?;
    reader.close()?;
    let buffer = std::rc::Rc::new(std::cell::RefCell::new(vec![JavaCodeUnit::from_u16(0)]));
    let array = match reader.look_ahead_char_array(Some(&buffer)) {
        Err(error) if error.is_instance(&["IOException"]) => "IOException",
        Err(error) => panic!("unexpected array lookahead error: {error:?}"),
        Ok(_) => "accepted",
    };
    let scalar = match reader.look_ahead() {
        Err(error) if error.is_instance(&["IOException"]) => "IOException",
        Err(error) => panic!("unexpected scalar lookahead error: {error:?}"),
        Ok(_) => "accepted",
    };
    println!("reader-lookahead={array}|{scalar}");
    println!(
        "delimiter-multi={}",
        values(&parse_one("left||\"x||y\"\r\n", format("||", None)?)?)
    );
    println!(
        "escaped={}",
        values(&parse_one(
            "\"a\\\"b\",c\\,d\r\n",
            format_details(",", None, false, Some(JavaCodeUnit::from_u16('\\' as u16)))?
        )?)
    );
    Ok(())
}

fn main() {
    run().unwrap();
}
