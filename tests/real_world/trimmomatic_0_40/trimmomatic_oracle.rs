use new_java2rust_focused_output::*;

type RecordHandle = std::rc::Rc<std::cell::RefCell<FastqRecord>>;

fn record(sequence: &str, quality: &str) -> Result<RecordHandle, JavaException> {
    Ok(std::rc::Rc::new(std::cell::RefCell::new(
        FastqRecord::new_string_string_string_string_int(
            "read".to_string(),
            Some(sequence.to_string()),
            "comment".to_string(),
            Some(quality.to_string()),
            33,
        )?,
    )))
}

fn visible(record: Option<RecordHandle>) -> String {
    let Some(record) = record else {
        return "<null>".to_string();
    };
    format!(
        "{}|{}|{}",
        FastqRecord::get_sequence_with_outer(record.clone()).unwrap(),
        FastqRecord::get_quality_with_outer(record.clone()).unwrap(),
        FastqRecord::get_head_pos_with_outer(record),
    )
}

fn pair_visible(records: Vec<Option<RecordHandle>>) -> String {
    let first = records[0].clone();
    let second = records[1].clone();
    let route = match (first.is_some(), second.is_some()) {
        (true, true) => "P",
        (true, false) => "U1",
        (false, true) => "U2",
        (false, false) => "D",
    };
    format!("{route}:{}:{}", visible(first), visible(second))
}

fn round_trip(input_path: &str, output_path: &str) -> Result<(), JavaException> {
    let parser = std::rc::Rc::new(std::cell::RefCell::new(FastqParser::new(33)));
    parser.borrow_mut().open(File::new(input_path))?;

    let mut serializer = FastqSerializer::new();
    serializer.open_file(File::new(output_path))?;
    let mut written = 0;
    while FastqParser::has_next_with_outer(parser.clone()) {
        let mut record = FastqParser::next_with_outer(parser.clone())?;
        record = LeadingTrimmer::new_int(20).process_record(record)?;
        if record.is_some() {
            record = TrailingTrimmer::new_int(20).process_record(record)?;
        }
        if record.is_some() {
            record = MinLenTrimmer::new_int(2).process_record(record)?;
        }
        if record.is_some() {
            serializer.write_record(record)?;
            written += 1;
        }
    }
    parser.borrow().close()?;
    serializer.close()?;
    println!("roundtrip={written}");
    Ok(())
}

fn malformed(input_path: &str) -> Result<(), JavaException> {
    let mut parser = FastqParser::new(33);
    match parser.open(File::new(input_path)) {
        Err(error) if error.is_instance(&["RuntimeException"]) => {
            println!("malformed=RuntimeException")
        }
        Err(error) => panic!("unexpected malformed FASTQ error: {error:?}"),
        Ok(()) => println!("malformed=accepted"),
    }
    Ok(())
}

fn run() -> Result<(), JavaException> {
    let low_ends = record("ACGTAC", "!!II!!")?;
    println!(
        "leading={}",
        visible(LeadingTrimmer::new_int(20).process_record(Some(low_ends.clone()))?)
    );
    println!(
        "trailing={}",
        visible(TrailingTrimmer::new_int(20).process_record(Some(low_ends.clone()))?)
    );
    println!(
        "crop={}",
        visible(CropTrimmer::new_int(4).process_record(Some(low_ends.clone()))?)
    );
    println!(
        "crop-long={}",
        visible(CropTrimmer::new_int(20).process_record(Some(low_ends.clone()))?)
    );
    println!(
        "minlen-keep={}",
        visible(MinLenTrimmer::new_int(6).process_record(Some(low_ends.clone()))?)
    );
    println!(
        "minlen-drop={}",
        visible(MinLenTrimmer::new_int(7).process_record(Some(low_ends))?)
    );
    println!(
        "leading-drop={}",
        visible(LeadingTrimmer::new_int(20).process_record(Some(record("AC", "!!")?))?)
    );
    println!(
        "trailing-empty={}",
        visible(TrailingTrimmer::new_int(20).process_record(Some(record("", "")?))?)
    );
    println!(
        "sliding={}",
        visible(
            SlidingWindowTrimmer::new_int_float(2, 20.0)
                .process_record(Some(record("ACGTAC", "IIII!!")?))?,
        )
    );
    println!(
        "sliding-short={}",
        visible(
            SlidingWindowTrimmer::new_int_float(4, 20.0)
                .process_record(Some(record("AC", "II")?))?,
        )
    );

    let mut chained = Some(record("ACGTACGT", "!!IIII!!")?);
    chained = LeadingTrimmer::new_int(20).process_record(chained)?;
    chained = TrailingTrimmer::new_int(20).process_record(chained)?;
    chained = CropTrimmer::new_int(3).process_record(chained)?;
    chained = MinLenTrimmer::new_int(3).process_record(chained)?;
    println!("chain={}", visible(chained));

    println!(
        "pair-keep={}",
        pair_visible(
            MinLenTrimmer::new_int(2)
                .process_records(Some(&[Some(record("AC", "II")?), Some(record("GT", "II")?)]))?
                .unwrap(),
        )
    );
    println!(
        "pair-u1={}",
        pair_visible(
            MinLenTrimmer::new_int(2)
                .process_records(Some(&[Some(record("AC", "II")?), Some(record("G", "I")?)]))?
                .unwrap(),
        )
    );
    println!(
        "pair-u2={}",
        pair_visible(
            MinLenTrimmer::new_int(2)
                .process_records(Some(&[Some(record("A", "I")?), Some(record("GT", "II")?)]))?
                .unwrap(),
        )
    );
    println!(
        "pair-drop={}",
        pair_visible(
            MinLenTrimmer::new_int(2)
                .process_records(Some(&[Some(record("A", "I")?), Some(record("G", "I")?)]))?
                .unwrap(),
        )
    );
    println!(
        "pair-leading={}",
        pair_visible(
            LeadingTrimmer::new_int(20)
                .process_records(Some(&[
                    Some(record("ACGT", "!!II")?),
                    Some(record("TGCA", "I!!!")?),
                ]))?
                .unwrap(),
        )
    );

    match record("AC", "!") {
        Err(error) if error.is_instance(&["RuntimeException"]) => {
            println!("mismatch=RuntimeException")
        }
        Err(error) => panic!("unexpected mismatch error: {error:?}"),
        Ok(_) => println!("mismatch=accepted"),
    }

    let args: Vec<String> = std::env::args().collect();
    round_trip(&args[1], &args[2])?;
    malformed(&args[3])?;
    Ok(())
}

fn main() {
    run().unwrap();
}
