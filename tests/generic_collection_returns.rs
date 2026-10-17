#[path = "common/mod.rs"]
mod common;

#[test]
fn specialized_trait_collection_returns_adapt_plain_and_result_payloads() {
    let translation = new_java2rust::translate_source(
        r#"
import java.util.*;
interface Values<T, Unused> {
    Set<T> values();
    Set<T> checkedValues(boolean fail);
}
class Words implements Values<String, Integer> {
    public Set<String> values() {
        return new HashSet<String>(Arrays.asList("a", "b", "a"));
    }
    public Set<String> checkedValues(boolean fail) {
        if (fail) throw new IllegalArgumentException("failed");
        return values();
    }
    public Set<String> keySet() { return values(); }
    public Set<String> entrySet() { return this.values(); }
}
class StoredWords implements Values<String, Integer> {
    private Set<String> items;
    StoredWords() { items = new HashSet<String>(Arrays.asList("a", "b", "a")); }
    public Set<String> values() { return items; }
    public Set<String> checkedValues(boolean fail) {
        if (fail) throw new IllegalArgumentException("failed");
        return values();
    }
}
class Use {
    static <T, U> int count(Values<T, U> source) { return source.values().size(); }
    static <T, U> int checked(Values<T, U> source, boolean fail) {
        return source.checkedValues(fail).size();
    }
    static int run() { return count(new Words()); }
    static int checkedRun(boolean fail) { return checked(new Words(), fail); }
    static int storedRun() { return count(new StoredWords()); }
    static int storedChecked(boolean fail) { return checked(new StoredWords(), fail); }
    static int sourceNames() {
        Words source = new Words();
        return source.keySet().size() + source.entrySet().size()
            + new HashSet<String>(source.values()).size();
    }
    static int nativeViews() {
        Map<Integer, String> map = new HashMap<Integer, String>();
        map.put(1, "a"); map.put(2, "a"); map.put(3, "b");
        Set<String> values = new HashSet<String>(map.values());
        List<Integer> keys = new ArrayList<Integer>(map.keySet());
        return values.size() * 10 + keys.size();
    }
}
"#,
    );
    common::cargo_test_generated_modules(
        "generic_collection_return_contracts",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn specialized_set_contract_preserves_keys_and_errors() {
    assert_eq!(Use::run(), 2);
    assert_eq!(Use::checked_run(false).unwrap(), 2);
    assert!(Use::checked_run(true).is_err());
    assert_eq!(Use::stored_run(), 2);
    assert_eq!(Use::stored_checked(false).unwrap(), 2);
    assert!(Use::stored_checked(true).is_err());
    assert_eq!(Use::source_names(), 6);
    assert_eq!(Use::native_views(), 23);
}
"#,
        }],
    );
}
