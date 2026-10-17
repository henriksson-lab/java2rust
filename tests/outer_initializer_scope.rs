use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_test_generated_modules};

#[test]
fn inner_initializers_read_inherited_outer_fields_in_class_scope() {
    let translation = translate_source(
        r#"
import java.util.AbstractList;
class CounterList extends AbstractList<String> {
    public String get(int index) { return ""; }
    public int size() { return 0; }
    void bump() { modCount++; }
    class Mark {
        int saved = modCount;
        Mark(int modCount) { int snapshot = saved; saved = snapshot; }
        int live() { return modCount; }
    }
    class AutoMark { int saved = modCount; }
    class LocalMark {
        int modCount = 7;
        int saved = modCount;
        LocalMark() { modCount = 8; }
        int current() { return modCount; }
    }
    static int probe() {
        CounterList list = new CounterList();
        list.bump();
        Mark first = list.new Mark(99);
        AutoMark second = list.new AutoMark();
        LocalMark local = list.new LocalMark();
        list.bump();
        if (local.current() != 8) return -1;
        return first.saved * 1000 + second.saved * 100 + local.saved * 10 + first.live();
    }
}
class SimpleOwner {
    int revision = 4;
    class AutoMark { int saved = revision; }
    static int probe() {
        SimpleOwner owner = new SimpleOwner();
        AutoMark mark = owner.new AutoMark();
        return mark.saved;
    }
}
class NameOwner {
    int Input = 3;
    class Entry {
        int input = 8;
        int saved = Input;
        Entry() { input = 9; }
        int current() { return input; }
    }
    static int probe() {
        NameOwner owner = new NameOwner();
        Entry entry = owner.new Entry();
        return entry.saved * 10 + entry.current();
    }
}
"#,
    );
    assert!(
        !translation.rust.contains("compile_error!"),
        "{}",
        translation.rust
    );
    cargo_test_generated_modules(
        "outer_initializer_scope",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[test]
fn inherited_outer_reads_preserve_snapshot_scope_and_live_identity() {
    assert_eq!(CounterList::probe(), 1172);
}
#[test]
fn implicit_inner_constructor_initializes_from_its_outer() {
    assert_eq!(SimpleOwner::probe(), 4);
}
#[test]
fn field_identity_is_case_sensitive() {
    assert_eq!(NameOwner::probe(), 39);
}
"#,
        }],
    );
}
