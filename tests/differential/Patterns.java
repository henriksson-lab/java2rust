/**
 * `instanceof` pattern variables must be bound everywhere JLS 6.3.1 puts them
 * in scope, not only where the type test is an `if` condition.
 *
 * The expression forms below used to drop the binding while keeping the type
 * test, so the pattern variable was emitted as an unresolved name (`E0425`) —
 * and the ternary additionally emitted raw Java (`s.length()`), which is 0019
 * leaking through the same hole.
 *
 * Java prints "3|3|3|3|true|true|3".
 *
 * Note the scope rule this fixture does *not* exercise: `boolean ok = o
 * instanceof String s;` followed by a use of `s` is rejected by javac, because
 * the binding is only in scope where the pattern is definitely matched.
 *
 * See claude_audits/0021-instanceof-pattern-variable-not-bound.md.
 */
public class Patterns {
    /** Statement form. */
    static int basic(Object o) {
        if (o instanceof String s) {
            return s.length();
        }
        return -1;
    }

    /** Negated scope: the binding is live after the early return. */
    static int negated(Object o) {
        if (!(o instanceof String s)) {
            return -1;
        }
        return s.length();
    }

    /** `&&` chain as an `if` condition. */
    static int andChain(Object o) {
        if (o instanceof String s && s.length() > 2) {
            return s.length();
        }
        return -1;
    }

    /** Ternary: the binding is in scope in the `then` branch. */
    static int ternary(Object o) {
        return (o instanceof String s) ? s.length() : -1;
    }

    /** `&&` in value position rather than as a condition. */
    static boolean valueAnd(Object o) {
        return o instanceof String s && s.length() == 3;
    }

    /**
     * `||` after a negated test: `s` is in scope in the right operand, because
     * that operand only runs when the left one is false.
     */
    static boolean valueOr(Object o) {
        return !(o instanceof String s) || s.length() > 2;
    }

    /** `while` condition: the binding is in scope in the body. */
    static int whileForm(Object o) {
        int n = 0;
        while (o instanceof String s) {
            n = s.length();
            break;
        }
        return n;
    }

    public static String run() {
        // A separate object per call on purpose: an erased `Object` lowers to
        // `Box<dyn Any>`, which is neither `Copy` nor `Clone`, so reusing one
        // local across five calls fails with `E0382` for reasons that have
        // nothing to do with pattern bindings.
        return basic("abc")
                + "|"
                + negated("abc")
                + "|"
                + andChain("abc")
                + "|"
                + ternary("abc")
                + "|"
                + valueAnd("abc")
                + "|"
                + valueOr("abc")
                + "|"
                + whileForm("abc");
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
