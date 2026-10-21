#[path = "common/mod.rs"]
mod common;

#[test]
fn public_generic_collection_wrappers_preserve_caller_visible_mutation() {
    let translation = new_java2rust::translate_source(
        r#"
import java.util.List;
class PublicForwarders {
    public static <T> void leaf(List<T> items) { items.set(0, items.get(1)); }
    public static <T> void middle(List<T> items) { leaf(items); }
    public static <T> void outer(List<T> items) { middle(items); }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_verify_generated_modules_with_edition(
        "public_collection_forwarding",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Done { fn check(self); }
impl Done for () { fn check(self) {} }
impl<E: std::fmt::Debug> Done for Result<(), E> { fn check(self) { self.unwrap(); } }
#[test] fn mutates_input() {
                let mut items = vec![String::from("first"), String::from("second")];
                PublicForwarders::outer(&mut items).check();
                assert_eq!(items, vec![String::from("second"), String::from("second")]);
            }"#,
        }],
        "test",
        "2021",
    );
}

#[test]
fn mutable_collection_forwarding_preserves_a_callee_retained_alias() {
    let translation = new_java2rust::translate_source(
        r#"
import java.util.List;
import java.util.ArrayList;
class RetainedItems {
    List<String> items;
    public void store(List<String> values) { items = values; }
}
class RetainedForwarding {
    public static <T> void change(List<T> values) { values.set(0, values.get(1)); }
    public static void forward(RetainedItems storage, List<String> values) {
        storage.store(values);
        change(values);
    }
    static int run() {
        RetainedItems storage = new RetainedItems();
        List<String> values = new ArrayList<String>();
        values.add("first"); values.add("second");
        forward(storage, values);
        if (storage.items != values) return 1;
        values.set(0, "updated");
        if (!storage.items.get(0).equals("updated")) return 2;
        return 0;
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_verify_generated_modules_with_edition(
        "retained_collection_forwarding",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Value { fn value(self) -> i32; }
impl Value for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Value for Result<i32, E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn retained_alias() { assert_eq!(RetainedForwarding::run().value(), 0); }
"#,
        }],
        "test",
        "2021",
    );
}

#[test]
fn forwarded_mutable_interface_fields_keep_one_shared_allocation() {
    let translation = new_java2rust::translate_source(
        r#"
interface Source { int next(); }
class Counter implements Source {
    int value;
    Owner owner;
    public int next() {
        if (owner.observe() != 7) return -100;
        return value++;
    }
}

class Owner {
    Source source;
    int[] domain = new int[] { 7 };
    Owner(Source source) { this.source = source; }
    public static int apply(Source source, int[] domain) { return source.next() + domain[0]; }
    public static int forward(Source source, int[] domain) { return apply(source, domain); }
    int take() { return forward(source, domain); }
    int observe() { return domain[0]; }
}
class SamplingContracts {
    static int run() {
        Counter counter = new Counter();
        Owner first = new Owner(counter);
        Owner alias = first;
        Owner second = new Owner(counter);
        counter.owner = first;
        if (first.take() != 7 || alias.take() != 8 || second.take() != 9) return 1;
        if (counter.next() != 3) return 2;
        return 0;
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_verify_generated_modules_with_edition(
        "sampling_contracts",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn shared_state() { assert_eq!(SamplingContracts::run().unwrap(), 0); }",
        }],
        "test",
        "2021",
    );
}

#[test]
fn lexical_super_and_uppercase_instance_fields_preserve_source_bindings() {
    let translation = new_java2rust::translate_source(
        r#"
class LabelBase {
    public String toString() { return "base"; }
}
class LabelChild extends LabelBase {
    public String toString() { return super.toString() + "/child"; }
}
class CacheOwner {
    static class Cache {
        final double[] LOG_VALUES;
        Cache(int size) {
            LOG_VALUES = new double[size];
            LOG_VALUES[1] = 3.5;
        }
        double read() { return this.LOG_VALUES[1] + LOG_VALUES[1]; }
    }
    static int run() {
        LabelChild child = new LabelChild();
        if (!child.toString().equals("base/child")) return 1;
        Cache cache = new Cache(3);
        if (cache.read() != 7.0) return 2;
        return 0;
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_verify_generated_modules_with_edition(
        "sampling_lexical_bindings",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: "#[test] fn source_bindings() { assert_eq!(CacheOwner::run().unwrap(), 0); }",
        }],
        "test",
        "2021",
    );
}
