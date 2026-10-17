use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_test_generated_modules};

#[test]
fn jdk_abstract_list_modification_counter_has_typed_inherited_storage() {
    let translation = translate_source(
        r#"
import java.util.AbstractList;
class CountingList extends AbstractList<String> {
    public String get(int index) { return ""; }
    public int size() { return 0; }
    int changed() { return ++modCount; }
    int changedOther(CountingList other) { return other.modCount++; }
}
"#,
    );
    assert!(
        translation.rust.contains("mod_count: i32"),
        "{}",
        translation
            .rust
            .lines()
            .filter(|line| line.contains("mod_count"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    cargo_test_generated_modules(
        "jdk_inherited_modification_counter",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn inherited_updates_preserve_java_prefix_postfix_and_wrapping() {
    let mut list = CountingList::new();
    list.mod_count = i32::MAX;
    assert_eq!(list.changed(), i32::MIN);
    let mut other = CountingList::new();
    other.mod_count = 5;
    assert_eq!(list.changed_other(&mut other), 5);
    assert_eq!(other.mod_count, 6);
}
"#,
        }],
    );
}
