#[path = "common/mod.rs"]
mod common;

#[test]
fn natural_order_factories_and_nullable_consumers_match_java() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/natural_order_consumers.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "natural_order_consumers",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn natural_order_values() {
    assert_eq!(NaturalOrderConsumers::compared().unwrap(), 89);
    assert_eq!(NaturalOrderConsumers::word_compared().unwrap(), -3);
    assert_eq!(NaturalOrderConsumers::reverse_word_compared().unwrap(), 3);
    assert_eq!(NaturalOrderConsumers::utf16_compared().unwrap(), -1987);
    assert_eq!(NaturalOrderConsumers::sorted().unwrap(), 259);
    assert_eq!(NaturalOrderConsumers::reverse_sorted().unwrap(), 952);
    assert_eq!(NaturalOrderConsumers::null_sorted().unwrap(), 259);
    assert_eq!(NaturalOrderConsumers::stored_null_sorted().unwrap(), 259);
    assert_eq!(NaturalOrderConsumers::nullable_sorted(false).unwrap(), 259);
    assert_eq!(NaturalOrderConsumers::nullable_sorted(true).unwrap(), 952);
    assert_eq!(NaturalOrderConsumers::extrema().unwrap(), 209);
    assert_eq!(NaturalOrderConsumers::empty_minimum().unwrap(), 7);
    assert_eq!(NaturalOrderConsumers::empty_sort().unwrap(), 0);
    assert_eq!(NaturalOrderConsumers::nullable_compare(false).unwrap(), 8);
    assert_eq!(NaturalOrderConsumers::nullable_compare(true).unwrap(), 1);
    assert_eq!(NaturalOrderConsumers::repeated_nullable(false).unwrap(), 202);
    assert_eq!(NaturalOrderConsumers::repeated_nullable(true).unwrap(), 909);
    assert_eq!(NaturalOrderConsumers::forwarded_null().unwrap(), 2);
}
"#,
        }],
    );
}

#[test]
fn natural_order_dispatches_source_comparable_and_preserves_exceptions() {
    let translation =
        new_java2rust::translate_source(include_str!("fixtures/natural_order_comparable.java"));
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "natural_order_comparable",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn source_compare_to_contract() {
    assert_eq!(NaturalComparableConsumers::no_eager(), 0);
    assert_eq!(NaturalComparableConsumers::compared().unwrap(), 7);
    assert_eq!(NaturalComparableConsumers::reversed().unwrap(), -7);
    assert_eq!(NaturalComparableConsumers::caught().unwrap(), 8);
    assert_eq!(NaturalComparableConsumers::caught_reversed().unwrap(), 8);
    assert_eq!(NaturalComparableConsumers::caught_sort().unwrap(), 8);
}
"#,
        }],
    );
}

#[test]
fn source_natural_factory_owners_take_precedence() {
    let translation = new_java2rust::translate_source(
        r#"
class Comparator {
    static int naturalOrder() { return 7; }
    static int reverseOrder() { return 3; }
}
class Collections {
    static int reverseOrder() { return 5; }
}
class SourceNaturalOwners {
    static int check() {
        return Comparator.naturalOrder() * 100 + Comparator.reverseOrder() * 10 + Collections.reverseOrder();
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
        "source_natural_owners",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn source_owners_are_not_native_factories() { assert_eq!(SourceNaturalOwners::check(), 735); }
"#,
        }],
    );
}

#[test]
fn natural_order_identity_strings_share_utf16_comparison() {
    let mut facts = new_java2rust::facts::FactSet::default();
    facts.java_string_identity_representation = true;
    let translation = new_java2rust::translate_source_with_facts(
        r#"
class IdentityStringNaturalOrder {
    static java.util.Comparator<String> order() { return java.util.Comparator.naturalOrder(); }
    static int compared() { return order().compare("😀", ""); }
}
"#,
        &facts,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "identity_string_natural_order",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn identity_string_order_is_utf16_not_utf8() {
    assert_eq!(IdentityStringNaturalOrder::compared().unwrap(), -1987);
    let left = JavaString::intern("😀");
    let right = JavaString::intern("\u{e000}");
    assert!(left < right);
    assert_eq!(left.java_compare_to_result(&right).unwrap(), -1987);
}
"#,
        }],
    );
}
