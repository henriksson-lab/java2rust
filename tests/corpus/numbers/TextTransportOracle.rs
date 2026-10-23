fn transport_observations() -> Vec<String> {
    fn units(value: &JavaString) -> String {
        value
            .units()
            .iter()
            .map(|unit| format!("{unit:04x}"))
            .collect()
    }
    fn bytes(value: &ByteArrayOutputStream) -> String {
        value
            .to_byte_array()
            .iter()
            .map(|byte| format!("{:02x}", *byte as u8))
            .collect()
    }
    let payloads: &[&[u16]] = &[
        &[],
        &[65],
        &[233],
        &[0xd83d, 0xde00],
        &[0xd800],
        &[0xdc00],
        &[0xd800, 0xd801],
        &[0xdc00, 0xdc01],
        &[0xd800, 65, 0xdc00],
        &[0xdc00, 0xd800],
        &[0],
    ];
    let mut out = Vec::new();
    for payload in payloads {
        let text = JavaString::from_units(payload.to_vec());
        let builder = StringBuilder::new_1(text.clone());
        let sequence = CharSequence::from(builder.clone());
        out.push(units(&sequence.to_string()));
        builder.append(JavaCodeUnit(0xd800));
        out.push(format!(
            "{}:{:x}",
            sequence.length(),
            sequence.char_at(sequence.length() - 1).as_u16()
        ));
        let slice = sequence.sub_sequence(0, text.len());
        builder.set_char_at_result(0, JavaCodeUnit(90)).unwrap();
        out.push(units(&slice.to_string()));
        let strings = StringWriter::new();
        let alias = Writer::from(strings.clone());
        strings.write(&text).unwrap();
        alias.write_chars(vec![JavaCodeUnit(0xd800)]).unwrap();
        let snapshot = strings.to_string();
        alias.write_chars(vec![JavaCodeUnit(0xdc00)]).unwrap();
        out.push(units(&snapshot));
        out.push(units(&strings.to_string()));
        alias.close().unwrap();
        alias.write_chars(vec![JavaCodeUnit(0xd800)]).unwrap();
        out.push(units(&strings.to_string()));
        let encoded_bytes = ByteArrayOutputStream::new();
        let encoded = OutputStreamWriter::new(encoded_bytes.clone());
        let alias = Writer::from(encoded.clone());
        encoded.write(&text).unwrap();
        encoded.flush().unwrap();
        out.push(bytes(&encoded_bytes));
        encoded.write_chars(vec![JavaCodeUnit(0xd800)]).unwrap();
        alias.flush().unwrap();
        out.push(bytes(&encoded_bytes));
        alias.write_chars(vec![JavaCodeUnit(0xdc00)]).unwrap();
        encoded.flush().unwrap();
        out.push(bytes(&encoded_bytes));
        alias.write_chars(vec![JavaCodeUnit(0xd800)]).unwrap();
        encoded.close().unwrap();
        out.push(bytes(&encoded_bytes));
        out.push(alias.write("x").unwrap_err().class_name().to_string());
        let printed = ByteArrayOutputStream::new();
        let printer = PrintStream::new(printed.clone());
        printer.print(text);
        printer.print(JavaCodeUnit(0xd800));
        printer.flush();
        out.push(bytes(&printed));
        printer.print(JavaCodeUnit(0xdc00));
        printer.flush();
        out.push(bytes(&printed));
        printer.print(JavaCodeUnit(0xd800));
        printer.close();
        out.push(bytes(&printed));
    }
    out
}
