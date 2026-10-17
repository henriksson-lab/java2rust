#[path = "common/mod.rs"]
mod common;

#[test]
fn emitted_child_interface_upcasts_execute_after_production_assembly() {
    use new_java2rust::BatchSource;
    let sources = [
        BatchSource::new(
            "example/api/Parent.java",
            "package example.api; public interface Parent { int value(); }",
        ),
        BatchSource::new(
            "example/api/Child.java",
            "package example.api; public interface Child extends Parent {}",
        ),
        BatchSource::new(
            "example/impl/Concrete.java",
            r#"
package example.impl;
import example.api.Child;
public class Concrete implements Child {
    private int count = 37;
    public int value() { return count; }
    public void increment() { count++; }
}
"#,
        ),
        BatchSource::new(
            "example/api/GenericParent.java",
            "package example.api; public interface GenericParent<T> { T item(); }",
        ),
        BatchSource::new(
            "example/api/GenericChild.java",
            "package example.api; public interface GenericChild<T> extends GenericParent<T> {}",
        ),
        BatchSource::new(
            "example/api/GenericLeaf.java",
            "package example.api; public interface GenericLeaf extends GenericChild<String> {}",
        ),
        BatchSource::new(
            "example/impl/GenericValue.java",
            r#"
package example.impl;
import example.api.GenericLeaf;
public class GenericValue implements GenericLeaf { public String item() { return "hello"; } }
"#,
        ),
        BatchSource::new(
            "example/client/Use.java",
            r#"
package example.client;
import example.api.Parent;
import example.api.Child;
import example.impl.Concrete;
import example.api.GenericParent;
import example.api.GenericLeaf;
import example.impl.GenericValue;
public class Use {
    public static Parent up(Child child) { return child; }
    public static int run() { return up(new Concrete()).value(); }
    public static int identity() {
        Concrete original = new Concrete();
        Parent parent = up(original);
        original.increment();
        return parent.value();
    }
    public static GenericParent<String> genericUp(GenericLeaf leaf) { return leaf; }
    public static int generic() { return genericUp(new GenericValue()).item().length(); }
}
"#,
        ),
    ];
    common::cargo_run_translated_batch_sources(
        "interface_upcast_assembly",
        &sources,
        "fn main() { assert_eq!(translated::Use::run(), 37); assert_eq!(translated::Use::identity(), 38); assert_eq!(translated::Use::generic(), 5); }",
    );
}
