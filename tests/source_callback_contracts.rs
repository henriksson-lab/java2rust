#[path = "common/mod.rs"]
mod common;

#[test]
fn packaged_overloaded_stream_predicates_propagate_checked_effects() {
    let translation = new_java2rust::translate_source(include_str!(
        "fixtures/checked_stream_predicate_effects.java"
    ));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "packaged_checked_stream_predicates",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait ProbeInt { fn value(self) -> i32; }
impl ProbeInt for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> ProbeInt for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn checked_predicates_execute_and_propagate() {
    assert_eq!(CheckedStreamEffects::run(false).value(), 1);
    assert_eq!(CheckedStreamEffects::run(true).value(), 7);
}
"#,
        }],
    );
}

#[test]
fn native_consumers_propagate_source_callback_contract_errors() {
    let translation = new_java2rust::translate_source(
        r#"
        import java.util.List;
        import java.util.ArrayList;
        import java.util.Collections;
        import java.util.Comparator;
        class NativeCallbackEffects {
            static void order(List<Integer> values, Comparator<Integer> comparator) {
                Collections.sort(values, comparator);
                Collections.sort(values, comparator);
            }
            static int compare(int a, int b, int flag) { if (flag < 0) throw new IllegalArgumentException(); return a - b; }
            static int probe(int flag) {
                List<Integer> values = new ArrayList<Integer>();
                values.add(2);
                values.add(1);
                order(values, (a, b) -> compare(a, b, flag));
                return values.get(0);
            }
        }
    "#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "source_callback_native_consumer",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn native_sort_executes_and_propagates_the_callback() {
    assert_eq!(NativeCallbackEffects::probe(1).unwrap(), 1);
    assert!(NativeCallbackEffects::probe(-1).is_err());
}
"#,
        }],
    );
}

#[test]
fn retained_callbacks_propagate_owned_lifetimes_through_forwarders() {
    let translation = new_java2rust::translate_source(
        r#"
        import java.util.Comparator;
        import java.util.PriorityQueue;
        class RetainedCallbacks {
            static PriorityQueue<Integer> queue(Comparator<Integer> comparator) { return new PriorityQueue<Integer>(comparator); }
            static PriorityQueue<Integer> forward(Comparator<Integer> comparator) { return queue(comparator); }
        }
    "#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "source_callback_retention",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn forwarded_comparator_survives_the_factory_call() {
    let mut queue = RetainedCallbacks::forward(|left, right| right - left);
    queue.push(1);
    queue.push(3);
    queue.push(2);
    assert_eq!(queue.remove(0), 3);
    assert_eq!(queue.remove(0), 2);
    assert_eq!(queue.remove(0), 1);
}
"#,
        }],
    );
}

#[test]
fn source_callbacks_share_error_carriers_and_source_alias_storage() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/source_callback_contracts.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "source_callback_error_contracts",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn errors_cross_formals_forwarders_and_reference_boundaries() {
    assert_eq!(SourceCallbackContracts::aliased().unwrap(), 4);
    for value in [0, 1] {
        assert_eq!(SourceCallbackContracts::lambda(value).unwrap(), value > 0);
        assert_eq!(SourceCallbackContracts::reference(value).unwrap(), value > 0);
        assert_eq!(SourceCallbackContracts::forwarded(value).unwrap(), value > 0);
        assert_eq!(SourceCallbackContracts::plain(value).unwrap(), value > 0);
    }
    assert!(SourceCallbackContracts::lambda(-1).is_err());
    assert!(SourceCallbackContracts::reference(-1).is_err());
    assert!(SourceCallbackContracts::forwarded(-1).is_err());
    assert!(!SourceCallbackContracts::caught(-1).unwrap());
    assert!(SourceCallbackContracts::caught(1).unwrap());
}
"#,
        }],
    );
}

#[test]
fn recursive_void_and_unused_callbacks_use_one_stable_effect_contract() {
    let translation = new_java2rust::translate_source(
        r#"
        import java.util.function.IntPredicate;
        import java.util.function.IntConsumer;
        class CallbackEffects {
            static boolean run(IntPredicate callback, int remaining, int value) {
                if (remaining <= 0) return callback.test(value);
                return run(callback, remaining - 1, value);
            }
            static boolean checked(int value) { if (value < 0) throw new IllegalArgumentException(); return value > 0; }
            static boolean positive(int value) { return value > 0; }
            static boolean direct(int value) { return run(item -> checked(item), 2, value); }
            static boolean reference(int value) { return run(CallbackEffects::checked, 2, value); }
            static boolean plainReference(int value) { return run(CallbackEffects::positive, 2, value); }
            static void perform(IntConsumer callback, int value) { callback.accept(value); }
            static void checkedVoid(int value) { if (value < 0) throw new IllegalArgumentException(); }
            static void voidBlock(int value) { perform(item -> { checkedVoid(item); }, value); }
            static void voidExpression(int value) { perform(item -> checkedVoid(item), value); }
            static boolean unused(IntPredicate callback) { return true; }
            static boolean forwardUnused(IntPredicate callback) { return unused(callback); }
            static boolean unusedThrow() { return unused(item -> { throw new IllegalArgumentException(); }); }
            static boolean forwardedUnused() { return forwardUnused(item -> checked(item)); }
        }
    "#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "source_callback_effects",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn callback_execution_controls_results_not_creation_or_forwarding() {
    assert!(CallbackEffects::direct(1).unwrap());
    assert!(!CallbackEffects::reference(0).unwrap());
    assert!(CallbackEffects::plain_reference(1).unwrap());
    assert!(CallbackEffects::direct(-1).is_err());
    assert!(CallbackEffects::reference(-1).is_err());
    CallbackEffects::void_block(1).unwrap();
    CallbackEffects::void_expression(1).unwrap();
    assert!(CallbackEffects::void_block(-1).is_err());
    assert!(CallbackEffects::void_expression(-1).is_err());
    assert!(CallbackEffects::unused_throw());
    assert!(CallbackEffects::forwarded_unused());
}
"#,
        }],
    );
}

#[test]
fn source_functional_lookalikes_keep_their_declared_method_and_return_type() {
    let translation = new_java2rust::translate_source(
        r#"
        class IntPredicate {
            int base;
            IntPredicate(int base) { this.base = base; }
            int test(int delta) { return base + delta; }
        }
        class Use {
            static int invoke(IntPredicate predicate) { return predicate.test(1); }
            static int probe() { return invoke(new IntPredicate(4)); }
        }
    "#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "source_callback_lookalike",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn source_declaration_wins_over_jdk_functional_spelling() {
    assert_eq!(Use::probe(), 5);
}
"#,
        }],
    );
}

#[test]
fn source_forwarding_preserves_inline_and_mutable_callback_contracts() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/source_callback_forwarding.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "source_callback_forwarding",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn forwarding_reborrows_the_same_callback_without_changing_its_effect() {
    assert!(SourceCallbackForwarding::plain(1));
    assert!(!SourceCallbackForwarding::plain(0));
    assert!(SourceCallbackForwarding::reference(1));
    assert!(!SourceCallbackForwarding::reference(0));
    assert!(!SourceCallbackForwarding::mutable(1));
}
"#,
        }],
    );
}
