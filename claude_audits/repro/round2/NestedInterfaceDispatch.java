/**
 * Interface dispatch through a `dyn` trait is fully implemented and correct --
 * the same program with the interface declared at top level translates, builds
 * and prints the right answer. The only thing broken here is that the
 * *coercion site* renders the nested interface with its Java dotted name:
 *
 *     let a: Box<dyn NestedInterfaceDispatchGreeter> =
 *         Box::new(...) as Box<dyn NestedInterfaceDispatch.Greeter>;
 *                                    ^ not a Rust path
 *
 * so the generated file does not even parse. Rewriting `dyn X.Y` to the mangled
 * name that the same line already uses on its left-hand side makes the file
 * compile and print the Java answer, default method included.
 *
 * Java prints
 *
 *     hi bob,salut luc
 *
 * This is [0013](done/0013-nested-type-names-unmangled-in-downcast.md) at a
 * second site.
 */
public class NestedInterfaceDispatch {
    interface Greeter {
        String name();

        default String greet() {
            return "hi " + name();
        }
    }

    static class En implements Greeter {
        public String name() {
            return "bob";
        }
    }

    static class Fr implements Greeter {
        public String name() {
            return "luc";
        }

        public String greet() {
            return "salut " + name();
        }
    }

    public static String run() {
        Greeter a = new En();
        Greeter b = new Fr();
        return a.greet() + "," + b.greet();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
