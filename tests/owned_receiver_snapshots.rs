#[path = "common/mod.rs"]
mod common;

#[test]
fn retained_array_snapshots_copy_once_before_index_side_effects() {
    let translation = new_java2rust::translate_source(
        r#"
class ArraySnapshot {
    int[] values = new int[] {7};
    void clear() { values = null; }
    int[] get() { return values; }
    int index() { values = new int[] {9}; return 0; }
    int read() { return get()[index()]; }
    static int readArgument(int[] values) { return values[0]; }
    static int run() {
        ArraySnapshot owner = new ArraySnapshot();
        int[] previous = owner.get();
        if (owner.read() != 7) return 1;
        if (readArgument(owner.get()) != 9) return 2;
        previous[0] = 11;
        if (readArgument(previous) != 11) return 3;
        owner.clear();
        try { owner.read(); return 4; }
        catch (NullPointerException expected) {}
        // The index executes before the captured null reference is checked.
        return readArgument(owner.get()) == 9 ? 0 : 5;
    }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    assert!(translation.rust.contains("__java_array_read_allocation"));
    for line in translation.rust.lines() {
        for capture in [
            "let __java_array_read_allocation = ",
            "let __java_array_allocation = ",
        ] {
            if let Some(value) = line.split(capture).nth(1) {
                let value = value.split(';').next().unwrap();
                assert!(
                    value.matches("::clone(").count() <= 1,
                    "array snapshot copied an already recovered allocation reference: {value}"
                );
            }
        }
    }
    common::cargo_test_generated_modules(
        "owned_array_snapshot",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
trait Exit { fn value(self) -> i32; }
impl Exit for i32 { fn value(self) -> i32 { self } }
impl<E: std::fmt::Debug> Exit for Result<i32,E> { fn value(self) -> i32 { self.unwrap() } }
#[test] fn snapshots() { assert_eq!(ArraySnapshot::run().value(), 0); }
"#,
        }],
    );
}

#[test]
fn fresh_interface_getter_result_is_moved_into_the_receiver_snapshot() {
    let translation = new_java2rust::translate_source(
        r#"
interface SnapshotProduct { int value(int delta); }
class SnapshotImplementation implements SnapshotProduct {
    int state;
    public int value(int delta) { state += delta; return state; }
}
class SnapshotHolder {
    SnapshotProduct stored;
    SnapshotHolder(SnapshotProduct product) { stored = product; }
    SnapshotProduct get() { return stored; }
    void clear() { stored = null; }
    int apply() { return get().value(1); }
    String text() { return "[" + get() + "]"; }
}
"#,
    );
    assert!(
        !translation.diagnostics.has_errors(),
        "{:?}",
        translation.diagnostics
    );
    common::cargo_test_generated_modules(
        "owned_receiver_snapshot",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[derive(Clone)]
struct CountedReference {
    copies: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    state: std::sync::Arc<std::sync::atomic::AtomicI32>,
}
impl SnapshotProduct for CountedReference {
    fn value(&mut self, delta: i32) -> i32 {
        self.state.fetch_add(delta, std::sync::atomic::Ordering::SeqCst) + delta
    }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn __java_reference_identity(&self) -> *const () {
        std::sync::Arc::as_ptr(&self.state).cast()
    }
    fn clone_any(&self) -> Box<dyn std::any::Any> { Box::new(std::clone::Clone::clone(self)) }
    fn clone_box(&self) -> Box<dyn SnapshotProduct> {
        self.copies.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Box::new(std::clone::Clone::clone(self))
    }
    fn to_string(&self) -> JavaLogicalString {
        self.state.load(std::sync::atomic::Ordering::SeqCst).to_string().into()
    }
}
#[test]
fn one_reference_copy_from_the_retained_field_and_no_copy_of_the_temporary() {
    let copies = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let state = std::sync::Arc::new(std::sync::atomic::AtomicI32::new(0));
    let mut holder = SnapshotHolder { stored: Some(Box::new(CountedReference {
        copies: copies.clone(), state: state.clone(),
    })) };
    assert_eq!(holder.apply().unwrap(), 1);
    assert_eq!(copies.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(holder.apply().unwrap(), 2);
    assert_eq!(copies.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert_eq!(state.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert_eq!(holder.text().unwrap(), "[2]".into());
    assert_eq!(copies.load(std::sync::atomic::Ordering::SeqCst), 3);
    holder.clear();
    assert_eq!(holder.text().unwrap(), "[null]".into());
    assert_eq!(copies.load(std::sync::atomic::Ordering::SeqCst), 3);
    assert!(holder.apply().unwrap_err().is_instance(&["NullPointerException"]));
}
"#,
        }],
    );
}
