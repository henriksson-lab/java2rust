use new_java2rust::{parser, resolve};

#[path = "common/mod.rs"]
mod common;

#[test]
fn upper_bounded_collection_reads_keep_the_stored_element_type() {
    common::verify_java_fixture_zero_contract(
        "wildcard_collection_reads",
        "WildcardCollectionReadProbe",
        "tests/fixtures/WildcardCollectionReadProbe.java",
        include_str!("fixtures/WildcardCollectionReadProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn reads() { assert_eq!(WildcardCollectionReadProbe::run().value(), 0); }
"#,
    );
}

#[test]
fn inferred_capture_preserves_shared_collection_contract_through_both_callees() {
    common::verify_java_fixture_zero_contract(
        "wildcard_collection_flow",
        "WildcardCollectionFlowProbe",
        "tests/fixtures/WildcardCollectionFlowProbe.java",
        include_str!("fixtures/WildcardCollectionFlowProbe.java"),
        r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn sharing() { assert_eq!(WildcardCollectionFlowProbe::run().value(), 0); }
"#,
    );
}

fn resolved(source: &str) -> new_java2rust::resolve::ResolvedProgram {
    let parsed = parser::parse_java_source(source);
    resolve::resolve_program(&parsed.program, parsed.diagnostics)
}

#[test]
fn captured_list_and_contravariant_model_are_inferred_in_either_argument_order() {
    let output = resolved(
        r#"
import java.util.List;
class CapturedObservation {}
class CapturedModel<T extends CapturedObservation> {}
class CapturedCalls {
    static <T extends CapturedObservation> void modelFirst(CapturedModel<? super T> model, List<T> values) {}
    static <T extends CapturedObservation> void listFirst(List<T> values, CapturedModel<? super T> model) {}
    static <O extends CapturedObservation> void probe(List<? extends O> values, CapturedModel<O> model) {
        modelFirst(model, values);
        listFirst(values, model);
    }
}
"#,
    );
    assert!(!output.diagnostics.has_errors(), "{:?}", output.diagnostics);
    assert!(!format!("{:?}", output.program).contains("JAVA_INFERENCE_CAPTURE"));
}

#[test]
fn capture_lower_bound_allows_insertion_and_unbounded_capture_allows_null() {
    let output = resolved(
        r#"
import java.util.List;
class CaptureWrites {
    static <T> void insert(List<T> values, T value) {}
    static void lower(List<? super String> values) { insert(values, "ok"); }
    static void nullable(List<?> values) { insert(values, null); }
}
"#,
    );
    assert!(!output.diagnostics.has_errors(), "{:?}", output.diagnostics);
}

#[test]
fn capture_inherits_its_source_owners_declared_upper_bound() {
    let output = resolved(
        r#"
class CaptureBase {}
class CaptureHolder<T extends CaptureBase> {}
class CaptureRead {
    static <T extends CaptureBase> void accept(CaptureHolder<T> value) {}
    static void probe(CaptureHolder<?> value) { accept(value); }
}

"#,
    );
    assert!(!output.diagnostics.has_errors(), "{:?}", output.diagnostics);
}

#[test]
fn captured_return_value_exposes_the_declared_upper_bound() {
    use new_java2rust::java_ir::{Expr, JavaType, Member, Stmt};
    let output = resolved(
        r#"
class CaptureBase {}
class CaptureHolder<T extends CaptureBase> {}
class CaptureRead {
    static <T extends CaptureBase> T read(CaptureHolder<T> value) { return null; }
    static CaptureBase probe(CaptureHolder<?> value) { return read(value); }
}
"#,
    );
    assert!(!output.diagnostics.has_errors(), "{:?}", output.diagnostics);
    let method = output
        .program
        .types
        .iter()
        .find(|ty| ty.name == "CaptureRead")
        .unwrap()
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "probe" => Some(method),
            _ => None,
        })
        .unwrap();
    let Stmt::Return(Some(Expr::Call {
        resolved: Some(call),
        ..
    })) = &method.body.as_ref().unwrap().statements[0]
    else {
        panic!("missing resolved return call")
    };
    assert_eq!(
        call.return_type,
        JavaType::Named {
            name: "CaptureBase".into(),
            args: vec![]
        }
    );
}

#[test]
fn captures_reject_unsafe_writes_and_independent_invariant_arguments() {
    for statement in [
        "insert(upper, number);",
        "insert(unknown, object);",
        "same(upper, other);",
        "same(upper, upper);",
        "insert(lower, text);",
        "UnsafeCapture.<Number>insert(upper, number);",
    ] {
        let output = resolved(&format!(
            r#"
import java.util.List;
class UnsafeCapture {{
    static <T> void insert(List<T> values, T value) {{}}
    static <T> void same(List<T> left, List<T> right) {{}}
    static void probe(List<? extends Number> upper, List<? extends Number> other,
                      List<?> unknown, List<? super Integer> lower,
                      Number number, Object object, String text) {{ {statement} }}
}}
"#
        ));
        assert!(
            output.diagnostics.has_errors(),
            "unsafe call was accepted: {statement}"
        );
        assert!(
            format!("{:?}", output.diagnostics).contains("resolve_call_ambiguous"),
            "{statement}: {:?}",
            output.diagnostics
        );
    }
}
