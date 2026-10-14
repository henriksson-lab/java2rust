use new_java2rust_focused_output::{
    JavaException, VCF3Parser, VCF4Parser, VCFHeaderLineCount, VCFHeaderLineType,
    VCFHeaderVersion, VCFPassThruTextTransformer, VCFPercentEncodedTextTransformer,
    VCFEncoder, VCFSimpleHeaderLine,
};

fn emit_map(label: &str, map: &std::collections::HashMap<String, String>, order: &[&str]) {
    let rendered = order
        .iter()
        .filter_map(|key| map.get(*key).map(|value| format!("{key}={value}")))
        .collect::<Vec<_>>()
        .join("|");
    println!("{label}={rendered}");
}

fn simple_exception_name(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

fn emit_failure<T>(label: &str, operation: impl FnOnce() -> Result<T, JavaException>) {
    match operation() {
        Ok(_) => println!("{label}=OK"),
        Err(failure) => println!("{label}={}", simple_exception_name(failure.class_name())),
    }
}

fn main() -> Result<(), JavaException> {
    let percent = VCFPercentEncodedTextTransformer::new();
    for input in ["plain", "%3D%41", "a%20b", "%GG", "%", "%25", "%09"] {
        println!(
            "percent:{input}={}",
            percent.decode_text_string(input.to_string())?
        );
    }
    println!(
        "percent-list={}",
        percent
            .decode_text_list(vec!["%41".to_string(), "x%2Cy".to_string()])?
            .join("|")
    );

    let pass = VCFPassThruTextTransformer::new();
    println!("pass={}", pass.decode_text_string("%41".to_string())?);

    for version in VCFHeaderVersion::values() {
        println!(
            "version={}:{}:{}:{}",
            version.name(),
            version.ordinal(),
            version.get_version_string().unwrap_or_default(),
            version.get_format_string().unwrap_or_default()
        );
    }
    println!(
        "version-parse={}",
        VCFHeaderVersion::get_header_version("##fileformat=VCFv4.3")?
            .map(|value| value.name().to_string())
            .unwrap_or_else(|| "null".to_string())
    );
    println!(
        "version-recent={}",
        VCFHeaderVersion::VCF4_3.is_at_least_as_recent_as(VCFHeaderVersion::VCF4_1)
    );
    emit_failure("version-invalid", || {
        VCFHeaderVersion::get_header_version("##fileformat=VCFv9.9")
    });

    for value in VCFHeaderLineCount::values() {
        println!("count={}:{}", value.name(), value.ordinal());
    }
    for value in VCFHeaderLineType::values() {
        println!("type={}:{}", value.name(), value.ordinal());
    }

    let expected = vec!["ID".to_string(), "Description".to_string()];
    let parse4 = VCF4Parser::new().parse_line_string_list(
        "<ID=q10,Description=\"quality, below ten\">".to_string(),
        expected.clone(),
    )?;
    emit_map("parse4", &parse4, &["ID", "Description"]);
    let parse3 = VCF3Parser::new().parse_line_string_list(
        "q10,\"quality, below ten\"".to_string(),
        expected.clone(),
    )?;
    emit_map("parse3", &parse3, &["ID", "Description"]);
    emit_failure("parse-unclosed", || {
        VCF4Parser::new().parse_line_string_list(
            "<ID=q10,Description=\"open>".to_string(),
            expected.clone(),
        )
    });
    emit_failure("parse-order", || {
        VCF4Parser::new().parse_line_string_list(
            "<Description=x,ID=q10>".to_string(),
            expected.clone(),
        )
    });

    let direct = VCFSimpleHeaderLine::new_string_string_string(
        Some("FILTER".to_string()),
        "q10".to_string(),
        "quality, below ten".to_string(),
    )?;
    println!("simple={}", direct.to_string()?);
    println!(
        "metadata={}:{}",
        direct.get_i_d().unwrap_or_else(|| "null".to_string()),
        direct
            .get_generic_fields()
            .get("Description")
            .cloned()
            .unwrap_or_else(|| "null".to_string())
    );

    let fields = std::collections::HashMap::from([
        ("ID".to_string(), "q20".to_string()),
        ("Description".to_string(), "has a \"quote\"".to_string()),
    ]);
    let from_map = VCFSimpleHeaderLine::new_string_map(Some("FILTER".to_string()), fields)?;
    println!("simple-map={}", from_map.to_string()?);

    let encoded = direct.to_string()?;
    let encoded_value = encoded.split_once('=').map(|(_, value)| value).unwrap_or("");
    let round_trip = VCFSimpleHeaderLine::new_string_v_c_f_header_version_string_list(
        encoded_value.to_string(),
        VCFHeaderVersion::VCF4_3,
        Some("FILTER".to_string()),
        expected,
    )?;
    println!(
        "roundtrip={}:{}:{}",
        round_trip.get_i_d().unwrap_or_else(|| "null".to_string()),
        round_trip
            .get_generic_fields()
            .get("Description")
            .cloned()
            .unwrap_or_else(|| "null".to_string()),
        round_trip.to_string()?
    );
    emit_failure("simple-invalid", || {
        VCFSimpleHeaderLine::new_string_string_string(
            Some("FILTER".to_string()),
            "bad<id".to_string(),
            "x".to_string(),
        )
    });

    for (index, value) in [0.0, 0.005, 0.01, 0.9996, 1.0, 12.345, -0.25]
        .into_iter()
        .enumerate()
    {
        println!(
            "record-double:{index}={}",
            VCFEncoder::format_v_c_f_double(value)
        );
    }
    let show_field = |value: Option<String>| value.unwrap_or_else(|| "null".to_string());
    println!(
        "record-field:null={}",
        show_field(VCFEncoder::format_v_c_f_field(None)?.map(|value| value.to_string()))
    );
    println!(
        "record-field:double={}",
        show_field(VCFEncoder::format_v_c_f_field(Some(Box::new(0.125f64)))?)
    );
    println!(
        "record-field:true={}",
        show_field(VCFEncoder::format_v_c_f_field(Some(Box::new(true)))?)
    );
    println!(
        "record-field:false={}",
        show_field(VCFEncoder::format_v_c_f_field(Some(Box::new(false)))?)
    );
    println!(
        "record-field:string={}",
        show_field(VCFEncoder::format_v_c_f_field(Some(Box::new(
            "A,C".to_string()
        )))?)
    );
    println!(
        "record-count={}",
        VCFEncoder::count_occurrences(',', &"A,C,,G".to_string())
    );
    for value in [".", ".,.", ".,", "A", ""] {
        println!(
            "record-missing:{value}={}",
            VCFEncoder::is_missing_value(&value.to_string())
        );
    }

    Ok(())
}
