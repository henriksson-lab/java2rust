#[path = "common/mod.rs"]
mod common;

#[test]
fn inherited_generic_bodies_retain_lexical_private_member_access_across_modules() {
    let sources = vec![
        new_java2rust::BatchSource::new(
            "Storage.java",
            r#"
package neutral.visibility;
public class Storage<Item> {
    private int count;
    private static final int OFFSET = 3;
    public Storage() { count = 4; }
    public int readOther(Storage<Item> other) { return other.count + OFFSET; }
}
"#,
        ),
        new_java2rust::BatchSource::new(
            "Child.java",
            r#"
package neutral.visibility;
class Middle<Unused, Payload> extends Storage<Payload> {}
public class Child<Item> extends Middle<Integer, Item> {}
"#,
        ),
    ];
    common::cargo_run_translated_batch_sources(
        "inherited_lexical_visibility",
        &sources,
        r#"
fn main() {
    let child = translated::Child::<String>::new();
    assert_eq!(child.read_other(translated::Storage::<String>::new()), 7);
}
"#,
    );
}

#[test]
fn unrelated_private_members_do_not_gain_external_visibility() {
    let translated = new_java2rust::translate_source(
        "class Isolated { private int count; private static final int OFFSET = 3; }",
    );
    assert!(translated.rust.contains("    count: i32,"));
    assert!(translated.rust.contains("    const OFFSET: i32 = 3;"));
    assert!(!translated.rust.contains("pub(crate) count:"));
    assert!(!translated.rust.contains("pub(crate) const OFFSET:"));
}

#[test]
fn nested_generic_subclasses_share_the_inherited_visibility_policy() {
    let translated = new_java2rust::translate_source(
        r#"
class Parent<Item> {
    private int count;
    private static final int OFFSET = 3;
    int readOther(Parent<Item> other) { return other.count + OFFSET; }
}
class Container { static class Nested<Item> extends Parent<Item> {} }
"#,
    );
    assert!(translated.rust.contains("pub(crate) count: i32,"));
    assert!(
        translated
            .rust
            .contains("pub(crate) const OFFSET: i32 = 3;")
    );
    assert!(!translated.rust.contains("pub count: i32,"));
    assert!(!translated.rust.contains("pub const OFFSET: i32 = 3;"));
}
