use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;

#[test]
fn interface_defaults_share_method_capabilities_without_constraining_handles() {
    let translation = translate_source(
        r#"
import java.util.List;
interface Repeater<Unused, T> {
    default List<T> duplicate(T value) {
        List<T> result = new java.util.ArrayList<T>();
        result.add(value);
        result.add(value);
        return result;
    }
    default List<T> again(T value) { return duplicate(value); }
    default int distinct(T value) {
        java.util.Set<T> result = new java.util.HashSet<T>();
        result.add(value); result.add(value);
        return result.size();
    }
    default <T> List<T> shadow(T value) {
        List<T> result = new java.util.ArrayList<T>();
        result.add(value); result.add(value);
        return result;
    }
}
class Repeated<Unused, T> implements Repeater<Unused, T> {}
interface Specialized<Spare, Payload> extends Repeater<Spare, Payload> {}
class SpecializedRepeated<Spare, Payload> implements Specialized<Spare, Payload> {}
interface Flipped<Payload, Spare> extends Specialized<Spare, Payload> {}
class Transposed<Unused, Data> implements Flipped<Data, Unused> {}
class Calls {
    static <Unused, T> List<T> via(Repeater<Unused, T> source, T value) {
        return source.again(value);
    }
}
"#,
    );
    common::cargo_test_generated_modules(
        "interface_default_capabilities",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
struct NotClone;
#[derive(Clone)]
struct Handle;
impl Repeater<NotClone, NotClone> for Handle {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn clone_box(&self) -> Box<dyn Repeater<NotClone, NotClone>> { Box::new(self.clone()) }
}
#[test]
fn defaults_and_forwarders_copy_only_the_payload() {
    let handle: Box<dyn Repeater<NotClone, NotClone>> = Box::new(Handle);
    let _ = handle.clone();
    let value = Repeated::<NotClone, String>::new();
    assert_eq!(value.duplicate("a".to_string()), vec!["a", "a"]);
    assert_eq!(value.shadow(7), vec![7, 7]);
    let erased: Box<dyn Repeater<NotClone, String>> = Box::new(value);
    assert_eq!(erased.again("b".to_string()), vec!["b", "b"]);
    assert_eq!(erased.distinct("b".to_string()), 1);
    assert_eq!(Calls::via::<NotClone, String>(&*erased, "c".to_string()), vec!["c", "c"]);
    let shared = std::rc::Rc::new(std::cell::RefCell::new(Repeated::<NotClone, String>::new()));
    assert_eq!(Repeater::again(&shared, "d".to_string()), vec!["d", "d"]);
    let specialized: Box<dyn Specialized<NotClone, String>> = Box::new(SpecializedRepeated::<NotClone, String>::new());
    assert_eq!(specialized.again("e".to_string()), vec!["e", "e"]);
    assert_eq!(specialized.distinct("e".to_string()), 1);
    let transposed: Box<dyn Flipped<String, NotClone>> = Box::new(Transposed::<NotClone, String>::new());
    assert_eq!(transposed.duplicate("f".to_string()), vec!["f", "f"]);
    assert_eq!(transposed.distinct("f".to_string()), 1);
}
"#,
        }],
    );
}

#[test]
fn generic_static_caller_inherits_bounds_from_a_constructed_receiver_method() {
    let translation = translate_source(
        r#"
import java.util.List;
interface ValueSource<T> { T read(); }
class Reader<Unused, T> {
    ValueSource<T> source;
    Reader(ValueSource<T> source) { this.source = source; }
    List<T> twice() {
        T value = source.read();
        List<T> result = new java.util.ArrayList<T>();
        result.add(value);
        result.add(value);
        return result;
    }
}
class Copier<T> {
    List<T> twice(T value) {
        List<T> result = new java.util.ArrayList<T>();
        result.add(value);
        result.add(value);
        return result;
    }
    static <U> List<U> viaNew(U value) {
        return new Copier<U>().twice(value);
    }
    static <Unused, U> List<U> viaSource(ValueSource<U> source) {
        return new Reader<Unused, U>(source).twice();
    }
}
"#,
    );
    common::cargo_test_generated_modules(
        "generic_constructed_receiver_bounds",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
struct NotClone;
fn check_source<T: Clone + 'static>(source: &dyn ValueSource<T>) {
    let _ = Copier::<NotClone>::via_source::<NotClone, T>(source);
}
#[derive(Clone)]
struct Words;
impl ValueSource<String> for Words {
    fn read(&self) -> String { "source".to_string() }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn clone_box(&self) -> Box<dyn ValueSource<String>> { Box::new(self.clone()) }
}
#[test]
fn static_caller_copies_only_the_method_payload() {
    let _: fn(&dyn ValueSource<String>) = check_source::<String>;
    assert_eq!(Copier::<NotClone>::via_source::<NotClone, String>(&Words), vec!["source".to_string(), "source".to_string()]);
    assert_eq!(Copier::<NotClone>::via_new("value".to_string()), vec!["value".to_string(), "value".to_string()]);
}
"#,
        }],
    );
}

#[test]
fn cloning_interface_fields_and_wrappers_does_not_clone_their_type_arguments() {
    let translation = translate_source(
        r#"
import java.util.List;
interface Source<T> { int size(); }
interface HandleView<T> { Source<T> getSource(); }
interface PayloadView<T> { T getValue(); }
interface Action<T> { List<T> duplicate(T value); }
class Repeated<T> implements Action<T> {
    public List<T> duplicate(T value) {
        List<T> result = new java.util.ArrayList<T>();
        result.add(value);
        result.add(value);
        return result;
    }
}
class HandleBox<T> implements HandleView<T> {
    Source<T> source;
    HandleBox(Source<T> source) { this.source = source; }
    Source<T> getSource() {
        Source<T> local = this.source;
        int ignored = local.size();
        return local;
    }
}
class InheritedHandle<T> extends HandleBox<T> {
    InheritedHandle(Source<T> source) { super(source); }
}
class ReboundHandle<Unused, Payload> extends HandleBox<Payload> {
    ReboundHandle(Source<Payload> source) { super(source); }
}
class TransposedHandle<First, Second> extends ReboundHandle<Second, First> {
    TransposedHandle(Source<First> source) { super(source); }
}
class PayloadBox<T> implements PayloadView<T> {
    T value;
    T getValue() { return this.value; }
}
class NamedPayloadBox<Integer> {
    Integer value;
    List<Integer> items;
    Integer getValue() { return this.value; }
    List<Integer> getItems() { return this.items; }
    java.lang.Integer literal;
    java.lang.Integer getLiteral() { return this.literal; }
}
"#,
    );
    common::cargo_test_generated_modules(
        "generic_clone_capabilities",
        vec![common::GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
struct NotClone;
fn read_handle<T: 'static>(value: &HandleBox<T>) { let _ = value.get_source(); }
fn copy_handle<T: 'static>(value: &HandleBox<T>) -> HandleBox<T> { std::clone::Clone::clone(value) }
fn box_handle<T: 'static>(value: HandleBox<T>) -> Box<dyn HandleView<T>> { Box::new(value) }
fn box_inherited<T: 'static>(value: InheritedHandle<T>) -> Box<dyn HandleView<T>> { Box::new(value) }
fn box_rebound<T: 'static, U: 'static>(value: ReboundHandle<T, U>) -> Box<dyn HandleView<U>> { Box::new(value) }
fn box_transposed<T: 'static, U: 'static>(value: TransposedHandle<T, U>) -> Box<dyn HandleView<T>> { Box::new(value) }
fn read_payload<T: Clone + 'static>(value: &PayloadBox<T>) { let _ = value.get_value(); }
fn box_payload<T: Clone + 'static>(value: PayloadBox<T>) -> Box<dyn PayloadView<T>> { Box::new(value) }
fn box_repeated<T: Clone + 'static>(value: Repeated<T>) -> Box<dyn Action<T>> { Box::new(value) }
fn copy_repeated<T: 'static>(value: &Repeated<T>) -> Repeated<T> { std::clone::Clone::clone(value) }
fn read_named_payload<T: Clone + 'static>(value: &NamedPayloadBox<T>) {
    let _: Option<&T> = value.get_value();
    let _: Option<Vec<T>> = value.get_items();
    let _: Option<i32> = value.get_literal();
}
#[derive(Clone)]
struct Provider(std::rc::Rc<std::cell::Cell<i32>>);
impl Source<NotClone> for Provider {
    fn size(&self) -> i32 { self.0.get() }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn clone_box(&self) -> Box<dyn Source<NotClone>> { Box::new(self.clone()) }
}
#[test]
fn handle_and_payload_clone_requirements_are_distinct() {
    let _: fn(&HandleBox<NotClone>) = read_handle::<NotClone>;
    let _: fn(&HandleBox<NotClone>) -> HandleBox<NotClone> = copy_handle::<NotClone>;
    let _: fn(&PayloadBox<String>) = read_payload::<String>;
    let _: fn(PayloadBox<String>) -> Box<dyn PayloadView<String>> = box_payload::<String>;
    let _: fn(Repeated<String>) -> Box<dyn Action<String>> = box_repeated::<String>;
    let _: fn(&Repeated<NotClone>) -> Repeated<NotClone> = copy_repeated::<NotClone>;
    let _: fn(&NamedPayloadBox<String>) = read_named_payload::<String>;
    assert_eq!(box_repeated(Repeated::<String>::new()).duplicate("ok".to_string()), vec!["ok".to_string(), "ok".to_string()]);
    let state = std::rc::Rc::new(std::cell::Cell::new(3));
    let value = HandleBox::<NotClone>::new(Box::new(Provider(state.clone())));
    let copied = copy_handle(&value);
    state.set(7);
    assert_eq!(value.get_source().unwrap().size(), 7);
    let view = box_handle(copied);
    let copied_view = view.clone();
    state.set(9);
    assert_eq!(view.get_source().unwrap().size(), 9);
    assert_eq!(copied_view.get_source().unwrap().size(), 9);
    let inherited = box_inherited(InheritedHandle::<NotClone>::new(Box::new(Provider(state.clone()))));
    let copied_inherited = inherited.clone();
    state.set(11);
    assert_eq!(inherited.get_source().unwrap().size(), 11);
    assert_eq!(copied_inherited.get_source().unwrap().size(), 11);
    let rebound = box_rebound(ReboundHandle::<NotClone, NotClone>::new(Box::new(Provider(state.clone()))));
    state.set(13);
    assert_eq!(rebound.get_source().unwrap().size(), 13);
    let transposed = box_transposed(TransposedHandle::<NotClone, String>::new(Box::new(Provider(state.clone()))));
    state.set(17);
    assert_eq!(transposed.get_source().unwrap().size(), 17);
}
"#,
        }],
    );
}
