use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;

#[test]
fn collection_storage_operations_do_not_require_key_comparison() {
    let translation = translate_source(
        r#"
import java.util.*;
interface Provider<T> { Set<T> values(); }
class Use {
    static <T> int count(Provider<T> provider) {
        int count = 0;
        for (T value : provider.values()) count++;
        return count;
    }
    static LinkedHashSet<String> ordered() { return new LinkedHashSet<String>(); }
}
"#,
    );
    common::cargo_test_generated_modules(
        "collection_storage_capabilities",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
// Deliberately implements neither equality, ordering, hashing nor Clone.
struct Opaque;
#[test]
fn empty_storage_and_iteration_need_no_comparison() {
    let mut map = JavaSemanticHashMap::<Opaque, Opaque>::default();
    assert_eq!(map.len(), 0);
    assert!(map.is_empty());
    assert_eq!(map.keys().count(), 0);
    assert_eq!(map.values().count(), 0);
    assert_eq!(map.values_mut().count(), 0);
    assert_eq!(map.iter().count(), 0);
    map.retain(|_, _| true);
    map.clear();
    let _ = JavaSemanticHashMap::<Opaque, Opaque>::with_capacity(2);
    let mut hash = JavaSemanticHashSet::<Opaque>::default();
    assert!(hash.is_empty());
    assert_eq!(hash.len(), 0);
    assert_eq!(hash.iter().count(), 0);
    hash.retain(|_| true);
    hash.clear();
    let _ = JavaSemanticHashSet::<Opaque>::with_capacity(2);
    let mut tree = JavaSemanticTreeSet::<Opaque>::default();
    assert!(tree.is_empty());
    assert_eq!(tree.len(), 0);
    assert_eq!(tree.iter().count(), 0);
    tree.retain(|_| true);
    tree.clear();
    let mut linked = JavaLinkedHashSet::<Opaque>::default();
    assert!(linked.is_empty());
    assert_eq!(linked.len(), 0);
    assert_eq!(linked.iter().count(), 0);
    linked.clear();
    let _ = JavaLinkedHashSet::<Opaque>::with_capacity(2);
}

// Comparison operations must still use Java contracts, not Rust Eq/Hash/Ord.
struct Key(i32);
impl JavaMapKey for Key {
    fn java_map_key_eq(&self, other: &Self) -> bool { self.0 == other.0 }
}
impl JavaTreeKey for Key {
    fn java_tree_cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}
#[test]
fn comparison_operations_preserve_java_contracts() {
    let mut map = JavaSemanticHashMap::<Key, i32>::new();
    assert_eq!(map.insert(Key(2), 3), None);
    assert_eq!(map.insert(Key(2), 4), Some(3));
    assert_eq!(map.get(&Key(2)), Some(&4));
    assert_eq!(map.remove(&Key(2)), Some(4));
    let mut hash = JavaSemanticHashSet::<Key>::new();
    assert!(hash.insert(Key(2)));
    assert!(!hash.insert(Key(2)));
    assert!(hash.contains(&Key(2)));
    assert!(hash.remove(&Key(2)));
    let mut tree = JavaSemanticTreeSet::<Key>::new();
    assert!(tree.insert(Key(2)));
    assert!(tree.insert(Key(1)));
    assert!(!tree.insert(Key(2)));
    assert_eq!(tree.iter().map(|key| key.0).collect::<Vec<_>>(), vec![1, 2]);
    assert!(tree.contains(&Key(2)));
    assert!(tree.remove(&Key(2)));
    let mut linked = JavaLinkedHashSet::<Key>::new();
    assert!(linked.insert(Key(2)));
    assert!(linked.insert(Key(1)));
    assert!(!linked.insert(Key(2)));
    assert_eq!(linked.iter().map(|key| key.0).collect::<Vec<_>>(), vec![2, 1]);
    assert!(linked.contains(&Key(2)));
    assert!(linked.remove(&Key(2)));
}
"#,
        }],
    );
}
