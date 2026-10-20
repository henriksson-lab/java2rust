use new_java2rust::{java_ir::Member, parser::parse_java_source, resolve::resolve_program};

#[test]
fn reference_array_reads_seed_source_call_and_constructor_nullability_without_null_literals() {
    let parsed = parse_java_source(
        r#"class Sink { Sink(Object value) {} }
class Calls {
    static void reference(Object value) {}
    static void primitive(int value) {}
    static void row(Object[] value) {}
    static void run(Object[] values, Object[][] rows, int[] ints, boolean choose) {
        reference(values[0]);
        reference((Object) values[0]);
        reference(choose ? values[0] : new Object());
        row(rows[0]);
        primitive(ints[0]);
        new Sink(values[0]);
    }
}"#,
    );
    assert!(!parsed.diagnostics.has_errors());
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(!resolved.diagnostics.has_errors());
    let usage = new_java2rust::usage::analyze_usage(&resolved.program);
    let owner = resolved
        .program
        .types
        .iter()
        .find(|ty| ty.name == "Calls")
        .unwrap();
    let method = owner
        .members
        .iter()
        .find_map(|member| match member {
            Member::Method(method) if method.name == "run" => Some(method),
            _ => None,
        })
        .unwrap();
    let method_usage = usage.source_method_usage(owner, method).unwrap();
    let mut reference_calls = 0;
    let mut primitive_calls = 0;
    for call in method_usage.call_sites() {
        match call.name() {
            "reference" | "row" => {
                assert_eq!(call.nullable_source_call_arg_indices(), &[0]);
                reference_calls += 1;
            }
            "primitive" => {
                assert!(call.nullable_source_call_arg_indices().is_empty());
                primitive_calls += 1;
            }
            _ => {}
        }
    }
    assert_eq!(reference_calls, 4);
    assert_eq!(primitive_calls, 1);
    let sink = method_usage
        .constructor_call_sites()
        .iter()
        .find(|call| {
            call.resolved()
                .is_some_and(|target| target.declaring_type == "Sink")
        })
        .unwrap();
    assert_eq!(sink.nullable_source_call_arg_indices(), &[0]);
}
