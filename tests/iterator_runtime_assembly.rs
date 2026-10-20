#[path = "common/mod.rs"]
mod common;

#[test]
fn retained_collection_display_survives_production_assembly() {
    use new_java2rust::BatchSource;
    let sources = [
        BatchSource::new(
            "RetainedCollectionFormatting.java",
            include_str!("fixtures/retained_collection_formatting.java"),
        ),
        BatchSource::new(
            "OtherDisplayUse.java",
            "class OtherDisplayUse { static String render(java.util.List<String> values) { return \"other=\" + values; } }",
        ),
    ];
    common::cargo_run_translated_batch_sources(
        "retained_display_assembly",
        &sources,
        r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
fn main() { assert_eq!(translated::RetainedCollectionFormatting::run().value(), 0); }
"#,
    );
}

#[test]
fn iterator_support_and_fallible_defaults_survive_production_assembly() {
    use new_java2rust::BatchSource;
    let sources = [
        BatchSource::new(
            "FallibleStep.java",
            include_str!("fixtures/default_void_fallibility.java"),
        ),
        BatchSource::new(
            "CursorUse.java",
            include_str!("fixtures/iterator_runtime_assembly.java"),
        ),
    ];
    common::cargo_run_translated_batch_sources(
        "iterator_default_completion_assembly",
        &sources,
        r#"
use translated::{FallibleStep, JavaException};
#[derive(Clone)]
struct Step(bool);
impl FallibleStep for Step {
    fn execute(&self) -> Result<(), JavaException> {
        if self.0 { Err(JavaException::new("Exception", "step".to_string())) }
        else { Ok(()) }
    }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn clone_any(&self) -> Box<dyn std::any::Any> { Box::new(self.clone()) }
    fn clone_box(&self) -> Box<dyn FallibleStep> { Box::new(self.clone()) }
}
fn main() {
    assert_eq!(translated::CursorUse::run().unwrap(), 3);
    for fail in [false, true] {
        let step: Box<dyn FallibleStep> = Box::new(Step(fail));
        assert_eq!(step.run().is_err(), fail);
        assert!(step.guarded(false).is_ok());
        assert_eq!(step.guarded(true).is_err(), fail);
        assert!(step.branches(false).is_err());
        assert_eq!(step.branches(true).is_err(), fail);
        assert!(step.recovered().is_ok());
    }
}
"#,
    );
}
