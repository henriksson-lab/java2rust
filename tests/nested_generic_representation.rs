use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_check_generated_modules, cargo_test_generated_modules};

#[test]
fn source_named_set_retains_its_nominal_type_inside_generic_defaults() {
    let translation = translate_source(
        r#"
import java.util.List;
import java.util.ArrayList;
class Set<Item> {}
interface DomainSets<Item> {
    Set<Item> one();
    default List<Set<Item>> all() {
        List<Set<Item>> result = new ArrayList<>();
        result.add(one());
        return result;
    }
}
"#,
    );
    cargo_check_generated_modules(
        "source_named_nested_set",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
fn domain_sets<Item: 'static>(source: &dyn DomainSets<Item>) -> Vec<Set<Item>> {
    source.all()
}
"#,
        }],
    );
}

#[test]
fn generic_default_nested_set_values_keep_semantic_key_representation() {
    let translation = translate_source(
        r#"
import java.util.List;
import java.util.Set;
import java.util.ArrayList;
import java.util.Map;
interface Buckets<Item> {
    Set<Item> get(int index);
    default List<Set<Item>> all() {
        List<Set<Item>> result = new ArrayList<>();
        result.add(get(0));
        return result;
    }
}
interface Tables<Item> {
    Map<Item, Integer> get(int index);
    default List<Map<Item, Integer>> all() {
        List<Map<Item, Integer>> result = new ArrayList<>();
        result.add(get(0));
        return result;
    }
}
interface DeepBuckets<Item> {
    List<Set<Item>> row();
    default List<List<Set<Item>>> all() {
        List<List<Set<Item>>> result = new ArrayList<>();
        result.add(row());
        return result;
    }
}
"#,
    );
    cargo_test_generated_modules(
        "nested_generic_default_set_values",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
fn nested_sets<Item: 'static>(source: &dyn Buckets<Item>)
    -> Vec<JavaSemanticHashSet<Item>> {
    source.all()
}
fn nested_maps<Item: 'static>(source: &dyn Tables<Item>)
    -> Vec<JavaSemanticHashMap<Item, i32>> {
    source.all()
}
fn deeper_sets<Item: 'static>(source: &dyn DeepBuckets<Item>)
    -> Vec<Vec<JavaSemanticHashSet<Item>>> {
    source.all()
}
struct JavaOnly(i32);
impl JavaMapKey for JavaOnly {
    fn java_map_key_eq(&self, other: &Self) -> bool { self.0 == other.0 }
}
struct BucketSource;
impl Buckets<JavaOnly> for BucketSource {
    fn get(&self, _index: i32) -> JavaSemanticHashSet<JavaOnly> {
        [JavaOnly(7), JavaOnly(7)].into_iter().collect()
    }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn clone_box(&self) -> Box<dyn Buckets<JavaOnly>> { Box::new(BucketSource) }
}
#[test]
fn nested_defaults_preserve_java_key_equality_without_rust_key_traits() {
    let rows = BucketSource.all();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].len(), 1);
    assert!(rows[0].contains(&JavaOnly(7)));
}
"#,
        }],
    );
}

#[test]
fn source_interface_payloads_survive_nested_collection_and_nominal_boundaries() {
    let translation = translate_source(
        r#"
import java.util.List;
import java.util.Map;
import java.util.function.BiFunction;
interface Port<T> { int size(); }
class Packet<T> { T value; }
class Entry<K, V> { K key; V value; }
interface Factory { BiFunction<Port<String>, String, Port<String>> builder(); }
class PlainStore {
    Map<String, List<Port<String>>> entries;
    List<List<Port<String>>> rows;
    List<Port<String>[]> arrays;
    List<Map.Entry<String, Port<String>>> metadata;
}
class WrappedStore {
    List<Packet<Port<String>>> packets;
    List<Entry<String, Port<String>>> entries;
}
"#,
    );
    assert!(
        !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
    cargo_test_generated_modules(
        "nested_generic_representation",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn nested_reference_containers_keep_java_default_state() {
    let plain = PlainStore::new();
    assert!(plain.entries.is_none());
    assert!(plain.rows.is_none());
    assert!(plain.arrays.is_none());
    assert!(plain.metadata.is_none());
    assert!(WrappedStore::new().packets.is_none());
    assert!(WrappedStore::new().entries.is_none());
}
"#,
        }],
    );
}
