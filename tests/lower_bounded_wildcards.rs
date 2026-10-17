#[path = "common/mod.rs"]
mod common;

#[test]
fn lower_bounded_owned_sink_accepts_a_value_read_from_a_source_producer() {
    let translation = new_java2rust::translate_source(
        r#"
interface ValueSink<Value> { void put(Value value); }
interface ValueSource<Value> { Value get(); }
class TextCell implements ValueSink<String> {
    String value;
    public void put(String value) { this.value = value; }
}
class TextSource implements ValueSource<String> {
    public String get() { return "hello"; }
}
class ValueTransfer {
    static <Value> void forward(ValueSink<? super Value> sink, ValueSource<Value> source) {
        Value value = source.get();
        sink.put(value);
    }
    static String run() {
        TextCell cell = new TextCell();
        forward(cell, new TextSource());
        return cell.value;
    }
}
"#,
    );
    common::cargo_test_generated_modules(
        "lower_bounded_owned_sink",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn mutable_sink_updates_original_cell() { assert_eq!(ValueTransfer::run(), \"hello\"); }",
        }],
    );
}

#[test]
fn source_lower_bounded_wildcard_accepts_values_of_its_lower_bound() {
    let translation = new_java2rust::translate_source(
        r#"
interface Sizer<Value> { int size(Value value); }
class TextSizer implements Sizer<String> {
    public int size(String value) { return value.length(); }
}
class SizerUse {
    static <Value> int invoke(Sizer<? super Value> sizer, Value value) {
        return sizer.size(value);
    }
    static int run() { return invoke(new TextSizer(), "hello"); }
}
"#,
    );
    common::cargo_check_generated_modules(
        "source_lower_bounded_wildcard",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "",
        }],
    );
}
