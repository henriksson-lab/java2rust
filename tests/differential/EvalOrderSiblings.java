/**
 * The sibling evaluation orders that 0026's "Broader point" section asks be
 * checked systematically.
 *
 * Each of these is correct today. They are pinned anyway, because they share the
 * property that made 0026 hard to see: the generated code *reads* like a
 * faithful translation, so a regression here would look like nothing at all in a
 * diff, and none of them produces a compile error.
 *
 *   - argument evaluation order (JLS 15.12.4.2: strictly left to right);
 *   - operand order in a binary expression (JLS 15.7.1: left before right);
 *   - `&&` / `||` short-circuiting, checked by the side effects the
 *     *unevaluated* operand would have had.
 *
 * The last field is the accumulated side-effect counter, which is what makes the
 * short-circuit cases observable: `f() && t()` must leave `t`'s +10 unapplied and
 * `t() || f()` must leave `f`'s +100 unapplied, so a translation that evaluated
 * both operands would report 210 rather than 110.
 *
 * Java prints "123,12,false,true,110".
 *
 * See claude_audits/done/0026-assignment-evaluation-order-differs-from-java.md.
 */
public class EvalOrderSiblings {
    static int n = 0;

    static int bump() {
        n = n + 1;
        return n;
    }

    static boolean t() {
        n = n + 10;
        return true;
    }

    static boolean f() {
        n = n + 100;
        return false;
    }

    public static String run() {
        n = 0;
        String args = "" + bump() + bump() + bump();

        n = 0;
        int operands = bump() * 10 + bump();

        n = 0;
        boolean shortCircuitAnd = f() && t();
        boolean shortCircuitOr = t() || f();

        return args + "," + operands + "," + shortCircuitAnd + "," + shortCircuitOr + "," + n;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
