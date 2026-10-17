use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;

#[test]
fn collection_values_compose_java_equality_without_rust_structural_traits() {
    let translated = translate_source(
        r#"
import java.util.*;
class Item {
    public boolean equals(Object other) { return other instanceof Item; }
}
final class Child extends Item {}
class Use {
    static <T> boolean eraseOne(List<T> values, T value) { return values.remove(value); }
    static <T> int removeDistinct(List<T> values, List<T> removed) {
        Set<T> seen = new HashSet<T>();
        for (T value : removed) {
            if (seen.add(value)) eraseOne(values, value);
        }
        return values.size();
    }
    static int run() {
        Set<Item> first = new HashSet<Item>();
        first.add(new Item());
        Set<Item> second = new HashSet<Item>();
        second.add(new Child());
        List<Set<Item>> groups = new ArrayList<Set<Item>>();
        groups.add(first);
        return removeDistinct(groups, Arrays.asList(second, second));
    }
    static LinkedHashSet<String> ordered() { return new LinkedHashSet<String>(); }
}
"#,
    );
    common::cargo_test_generated_modules(
        "collection_value_java_equality",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translated.rust,
            tests: r#"
struct JavaOnly(i32);
impl JavaMapKey for JavaOnly {
    fn java_map_key_eq(&self, other: &Self) -> bool { self.0 == other.0 }
}
impl JavaTreeKey for JavaOnly {
    fn java_tree_cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}
fn set(values: &[i32]) -> JavaSemanticHashSet<JavaOnly> {
    values.iter().map(|value| JavaOnly(*value)).collect()
}
#[test]
fn java_list_removes_an_equal_set_value() { assert_eq!(Use::run(), 0); }
#[test]
fn set_values_compare_contents_not_encounter_order() {
    assert!(set(&[2, 1]).java_map_key_eq(&set(&[1, 2])));
    assert!(!set(&[2, 1]).java_map_key_eq(&set(&[1, 3])));
    let mut nested = JavaSemanticHashSet::new();
    assert!(nested.insert(set(&[2, 1])));
    assert!(!nested.insert(set(&[1, 2])));
    assert!(nested.contains(&set(&[1, 2])));
    assert!(nested.remove(&set(&[1, 2])));
    let left: JavaLinkedHashSet<JavaOnly> = [JavaOnly(2), JavaOnly(1)].into_iter().collect();
    let right: JavaLinkedHashSet<JavaOnly> = [JavaOnly(1), JavaOnly(2)].into_iter().collect();
    assert!(left.java_map_key_eq(&right));
    let tree: JavaSemanticTreeSet<JavaOnly> = [JavaOnly(2), JavaOnly(1)].into_iter().collect();
    let other: JavaSemanticTreeSet<JavaOnly> = [JavaOnly(1), JavaOnly(2)].into_iter().collect();
    assert!(tree.java_map_key_eq(&other));
}
#[test]
fn map_values_use_java_equality_for_keys_and_values() {
    let mut left = JavaSemanticHashMap::new();
    left.insert(JavaOnly(2), JavaOnly(20));
    left.insert(JavaOnly(1), JavaOnly(10));
    let mut right = JavaSemanticHashMap::new();
    right.insert(JavaOnly(1), JavaOnly(10));
    right.insert(JavaOnly(2), JavaOnly(20));
    assert!(left.java_map_key_eq(&right));
    right.insert(JavaOnly(2), JavaOnly(30));
    assert!(!left.java_map_key_eq(&right));
    let mut maps = JavaSemanticHashSet::new();
    assert!(maps.insert(left));
    assert!(maps.insert(right));
    assert_eq!(maps.len(), 2);
    let mut nullable = JavaSemanticHashMap::<JavaOnly, Option<JavaOnly>>::new();
    nullable.insert(JavaOnly(1), None);
    let mut missing = JavaSemanticHashMap::<JavaOnly, Option<JavaOnly>>::new();
    missing.insert(JavaOnly(2), None);
    // A null value is not evidence that an absent key is present.
    assert!(!nullable.java_map_key_eq(&missing));
    missing.remove(&JavaOnly(2));
    missing.insert(JavaOnly(1), None);
    assert!(nullable.java_map_key_eq(&missing));
    missing.insert(JavaOnly(1), Some(JavaOnly(3)));
    assert!(!nullable.java_map_key_eq(&missing));
}
"#,
        }],
    );
}
