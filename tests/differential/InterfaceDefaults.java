/**
 * Interface `default` methods, across every surface 0025 lists.
 *
 * Java's semantics: a class inherits a `default` body from every interface it
 * implements — directly, through a super-interface, or through a superclass's
 * `implements` — unless it declares its own. `Iface.super.m()` calls a specific
 * interface's default explicitly.
 *
 * The trait carried the default body correctly long before this fixture
 * existed; every failure here was somewhere else:
 *
 *   - no signature registered for an inherited default, so the call took the
 *     erased-display path and returned "" — compiling cleanly and silently
 *     producing the wrong answer;
 *   - a facade shell synthesised for a locally declared type, colliding with
 *     the type actually emitted for it (`E0592`, `E0428`);
 *   - a super-interface not lowered as a supertrait, so a default body could
 *     not call the methods it inherits (`E0599`);
 *   - `Iface.super.m()` falling through to a static-owner path and emitting
 *     `Super::greet()` (`E0433`).
 *
 * Java prints "hi a,yo,hi d,hi c!,hi e,hi a!,yo!".
 *
 * See claude_audits/done/0025-interface-default-methods-are-discarded.md.
 */
public class InterfaceDefaults {
    interface Greeter {
        String name();

        default String greet() {
            return "hi " + name();
        }

        /** A default calling another default. */
        default String loudGreet() {
            return greet() + "!";
        }
    }

    /** A super-interface: `C` inherits `greet` through `Polite`. */
    interface Polite extends Greeter {
        default String polite() {
            return greet() + "!";
        }
    }

    /** Inherits the default. */
    static class A implements Greeter {
        public String name() {
            return "a";
        }
    }

    /** Overrides it: the override must win. */
    static class B implements Greeter {
        public String name() {
            return "b";
        }

        public String greet() {
            return "yo";
        }
    }

    /** Inherits via its superclass's `implements`, not its own. */
    static class D extends A {
        public String name() {
            return "d";
        }
    }

    /** Inherits through a super-interface. */
    static class C implements Polite {
        public String name() {
            return "c";
        }
    }

    /** Overrides, then delegates back to the interface default explicitly. */
    static class E implements Greeter {
        public String name() {
            return "e";
        }

        public String greet() {
            return Greeter.super.greet();
        }
    }

    public static String run() {
        return new A().greet()
                + "," + new B().greet()
                + "," + new D().greet()
                + "," + new C().polite()
                + "," + new E().greet()
                + "," + new A().loudGreet()
                + "," + new B().loudGreet();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
