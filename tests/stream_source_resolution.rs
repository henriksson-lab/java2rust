use new_java2rust::facts::{
    ExternalApiEffectFact, ExternalApiRole, FactProvenance, FactSet, NullabilityState,
    ReceiverMutabilityFact, TypeCapabilityOverride,
};
use new_java2rust::java_ir::{CallKind, Expr, JavaType, ResolvedCall};
use new_java2rust::stream_shape::infer_stream_source_element_shape_with_facts;
use new_java2rust::types::TypeEnv;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_test_generated_modules};

fn named(name: &str, args: Vec<JavaType>) -> JavaType {
    JavaType::Named {
        name: name.to_string(),
        args,
    }
}

fn fallback_facts(source_owner: Option<&str>) -> FactSet {
    let mut facts = FactSet::default();
    facts.external_api_effect_facts.push(ExternalApiEffectFact {
        method_name: "values".to_string(),
        receiver_type: Some("neutral.Bucket".to_string()),
        arg_count: Some(0),
        parameter_patterns: None,
        role: ExternalApiRole::Getter,
        receiver_mutability: ReceiverMutabilityFact::ReadOnly,
        return_type: Some(named("List", vec![named("Object", vec![])])),
        return_nullability: NullabilityState::DefinitelyNonNull,
        thrown: vec![],
        provenance: FactProvenance::ExternalLibraryPack,
    });
    if let Some(owner) = source_owner {
        facts
            .type_capability_overrides
            .push(TypeCapabilityOverride {
                type_name: owner.to_string(),
                copy: None,
                clone: None,
                string_like: None,
                sequence_like: None,
                implements: vec![],
                extends: None,
                final_type: None,
                object_safe_trait: None,
                provenance: FactProvenance::ParsedSource,
            });
    }
    facts
}

fn values_call(resolved: bool, element: JavaType) -> Expr {
    Expr::Call {
        type_args: Vec::new(),
        receiver: Some(Box::new(Expr::Var("bucket".to_string()))),
        name: "values".to_string(),
        args: vec![],
        resolved: resolved.then(|| ResolvedCall {
            type_args: Vec::new(),
            declaring_type: "neutral.Bucket".to_string(),
            rust_name: "values".to_string(),
            kind: CallKind::Instance,
            return_type: named("List", vec![element]),
            arg_target_types: vec![],
            arg_coercions: vec![],
        }),
    }
}

#[test]
fn resolved_source_stream_producer_keeps_its_element_type_over_external_metadata() {
    let mut env = TypeEnv::new();
    env.insert_local("bucket", named("neutral.Bucket", vec![]));
    let facts = fallback_facts(Some("neutral.Bucket"));
    for element in [JavaType::String, named("T", vec![])] {
        let shape = infer_stream_source_element_shape_with_facts(
            &values_call(true, element.clone()),
            &env,
            &facts,
        )
        .unwrap();
        assert_eq!(shape.java_ty, element);
    }
    // Source ownership uses the same import identity policy as call planning.
    env.insert_static_type_alias("Bucket", "neutral.Bucket");
    let mut call = values_call(true, JavaType::String);
    if let Expr::Call {
        resolved: Some(resolved),
        ..
    } = &mut call
    {
        resolved.declaring_type = "Bucket".to_string();
    }
    assert_eq!(
        infer_stream_source_element_shape_with_facts(&call, &env, &facts)
            .unwrap()
            .java_ty,
        JavaType::String,
    );
}

#[test]
fn external_stream_producer_still_uses_its_external_contract() {
    let mut env = TypeEnv::new();
    env.insert_local("bucket", named("neutral.Bucket", vec![]));
    // A different source owner with the same leaf name must not steal this ABI.
    let facts = fallback_facts(Some("other.Bucket"));
    for resolved in [false, true] {
        let shape = infer_stream_source_element_shape_with_facts(
            &values_call(resolved, JavaType::String),
            &env,
            &facts,
        )
        .unwrap();
        assert_eq!(shape.java_ty, named("Object", vec![]));
    }
}

#[test]
fn resolved_non_sequence_source_return_does_not_inherit_a_facade_sequence() {
    let mut env = TypeEnv::new();
    env.insert_local("bucket", named("neutral.Bucket", vec![]));
    let mut call = values_call(true, JavaType::String);
    if let Expr::Call {
        resolved: Some(resolved),
        ..
    } = &mut call
    {
        resolved.return_type = JavaType::String;
    }
    assert!(
        infer_stream_source_element_shape_with_facts(
            &call,
            &env,
            &fallback_facts(Some("neutral.Bucket")),
        )
        .is_none()
    );
}

#[test]
fn imported_source_interface_stream_keeps_generic_elements_over_external_metadata() {
    let mut facts = fallback_facts(None);
    facts.external_api_effect_facts[0].return_type =
        Some(named("Set", vec![named("Object", vec![])]));
    let mut unqualified = facts.external_api_effect_facts[0].clone();
    unqualified.receiver_type = Some("Bucket".to_string());
    facts.external_api_effect_facts.push(unqualified);
    let translations =
        new_java2rust::translate_batch_sources_with_context_sources_and_summary_sources(
            &[
                new_java2rust::BatchSource::new(
                    "neutral/Bucket.java",
                    r#"
package neutral;
import java.util.Set;
public interface Bucket<T, U> { Set<T> values(); int score(T value); }
"#,
                ),
                new_java2rust::BatchSource::new(
                    "client/Use.java",
                    r#"
package client;
import neutral.Bucket;
public class Use {
    public static <T, U> int run(Bucket<T, U> bucket) {
        return bucket.values().stream().mapToInt(bucket::score).max().orElse(7);
    }
}
"#,
                ),
            ],
            &[],
            &[],
            false,
            &facts,
        );
    let client = translations
        .iter()
        .find(|item| item.path() == "client/Use.java")
        .unwrap();
    let client_impl = client
        .rust()
        .split("impl Use {")
        .nth(1)
        .expect(client.rust());
    let run = client_impl
        .lines()
        .find(|line| line.contains(".map("))
        .expect(client_impl);
    assert!(run.contains("bucket.score("), "{run}");
    assert!(!run.contains("Clone::clone(&value)"), "{run}");
    assert!(!run.contains("dyn std::any::Any"), "{run}");
    assert!(!run.contains("compile_error!"), "{run}");
    common::cargo_run_translated_batch_sources_with_facts(
        "imported_source_stream_precedence",
        &[
            new_java2rust::BatchSource::new(
                "neutral/Bucket.java",
                r#"
package neutral;
import java.util.Set;
public interface Bucket<T, U> { Set<T> values(); int score(T value); }
"#,
            ),
            new_java2rust::BatchSource::new(
                "neutral/Words.java",
                r#"
package neutral;
import java.util.*;
public class Words implements Bucket<String, String> {
    public Set<String> values() { return new HashSet<String>(Arrays.asList("a", "four", "xy")); }
    public int score(String value) { return value.length(); }
}
"#,
            ),
            new_java2rust::BatchSource::new(
                "client/Use.java",
                r#"
package client;
import neutral.Bucket;
import neutral.Words;
public class Use {
    public static <T, U> int run(Bucket<T, U> bucket) {
        return bucket.values().stream().mapToInt(bucket::score).max().orElse(7);
    }
    public static int probe() { return run(new Words()); }
}
"#,
            ),
        ],
        "fn main() { assert_eq!(translated::Use::probe().unwrap(), 4); }",
        "external-api values neutral.Bucket getter read-only non-null args=0 return=Set<Object>\nexternal-api values Bucket getter read-only non-null args=0 return=Set<Object>\n",
    );
}

#[test]
fn generic_source_collection_producer_runs_with_conflicting_fallback_facts() {
    let translation = new_java2rust::translate_source_with_facts(
        r#"
package neutral;
import java.util.*;
class Bucket<T> {
    private List<T> items;
    Bucket(List<T> items) { this.items = items; }
    List<T> values() { return items; }
}
class Measure { int score(String item) { return item.length(); } }
class Reused {
    private List<String> first;
    private List<String> second;
    Reused(List<String> items) { this.first = items; this.second = items; }
    int total() { return first.size() + second.size(); }
}
class Use {
    static int run() {
        Bucket<String> bucket = new Bucket<String>(Arrays.asList("a", "four", "xy"));
        Measure measure = new Measure();
        return bucket.values().stream().mapToInt(measure::score).max().orElse(7);
    }
    static int reused() { return new Reused(Arrays.asList("a", "b")).total(); }
}
"#,
        &fallback_facts(None),
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    cargo_test_generated_modules(
        "stream_source_return_precedence",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn typed_source_producer_feeds_the_typed_callback() {
    assert_eq!(Use::run().unwrap(), 4);
    assert_eq!(Use::reused().unwrap(), 4);
    struct NonClone;
    let _bucket = Bucket::new(std::rc::Rc::new(std::cell::RefCell::new(
        JavaNativeCollection::list(vec![NonClone], true)
    )));
}
"#,
        }],
    );
}
