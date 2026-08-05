/**
 * A nested class naming an enclosing class's `static` field.
 *
 * In Java an enclosing class's statics are in scope inside a nested class, and
 * they name the *same* variable — there is no instance involved, so it works
 * whether or not either class is `static`.
 *
 * Two defects sat on top of each other here:
 *
 *   - the enclosing type's fields were collected as an outer-*instance*
 *     capture without filtering out `static` ones, so a `static` nested class
 *     reading a `static` field was lowered as `self.__outer.borrow().n` —
 *     inside a method with no `self` at all;
 *   - once the capture was suppressed the read resolved through the shared
 *     shape environment, which carries every type's statics under their bare
 *     names, and the accessor projection hardcoded `Self::`. From inside `In`
 *     that emitted `NestedStaticIn::__static_n()` for a field `NestedStatic`
 *     owns (`E0599`).
 *
 * The write side already qualified with the declaring type, which is why the
 * read was the half that failed: assignment planning carries a `rust_owner`
 * and the bare-name read path did not.
 *
 * The cases below pin both directions (nested reads and writes an enclosing
 * static, enclosing reads a nested one) and a nested static of the same simple
 * name shadowing nothing, so `Self` and the qualified owner cannot be confused
 * for one another.
 *
 * Java prints "1,1,4,9,2".
 *
 * See claude_audits/done/0037-nested-class-outer-static-read-is-capture.md.
 */
public class NestedStatic {
    static int n = 0;
    static int shared = 2;

    /** `static` nested class touching the enclosing class's statics. */
    static class In {
        /** Its own static, same simple name as the enclosing one. */
        static int shared = 9;

        static int read() {
            return n;
        }

        static void bump() {
            n = n + 1;
        }

        /** `shared` here must be `In`'s, not the enclosing class's. */
        static int own() {
            return shared;
        }

        /** ... while the enclosing one is still reachable by qualifying. */
        static int outerShared() {
            return NestedStatic.shared;
        }
    }

    /** A second nested class, so the owner cannot be guessed from arity. */
    static class Other {
        static int quadrupled() {
            return NestedStatic.shared + In.own() - In.outerShared() - 5;
        }
    }

    public static String run() {
        In.bump();
        return In.read()
                + "," + n
                + "," + Other.quadrupled()
                + "," + In.shared
                + "," + shared;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
