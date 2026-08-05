/**
 * Java evaluates an assignment's *target* subexpressions before its right-hand
 * side; Rust evaluates the value first. The generated code reads like a faithful
 * translation and computes something else.
 *
 * The cases here are chosen so that each one *observes* the difference. That
 * matters more than it sounds: the obvious probe for the second case,
 *
 *     int[] b = new int[3]; int m = 0; b[m] = m++;
 *
 * leaves `b[0] == 0` under both orders, and only differs in **which element was
 * written** — Java writes `b[0]`, the unfixed translation writes `b[1]`. So the
 * fixture reads the whole array, not the slot it expects.
 *
 * Deliberately absent: `pick(e).field = calls`, where the receiver is a call
 * returning an object. That is wrong for a second, independent reason — the
 * argument is passed as `e.clone()`, so the write cannot be observed at all —
 * and a fixture that cannot pass for two reasons pins neither. It belongs with
 * claude_audits/0002-object-aliasing-has-no-representation.md.
 *
 * The two-dimensional case (`g.grid[f()][f()] = v`, already correct) is left
 * out on purpose: a `int[][]` field lowers to `ndarray`, which the differential
 * harness does not link, so including it would make this fixture fail for a
 * reason that has nothing to do with evaluation order. It is covered in the
 * audit instead.
 *
 * Java prints "1,0:0:0,15,1,1".
 *
 * See claude_audits/done/0026-assignment-evaluation-order-differs-from-java.md.
 */
public class EvalOrder {
    int field = 0;

    public static String run() {
        StringBuilder sb = new StringBuilder();

        // The index has the side effect: Java increments `k` before reading the
        // right-hand side, so the value stored is 1.
        int[] a = new int[3];
        int k = 0;
        a[k++] = k;
        sb.append(a[0]);

        // The *value* has the side effect, and it changes what the index
        // denotes. Java evaluates the index (0) first, so the write is to
        // `b[0]`; evaluating the value first makes it `b[1]`.
        int[] b = new int[3];
        int m = 0;
        b[m] = m++;
        sb.append(",").append(b[0]).append(":").append(b[1]).append(":").append(b[2]);

        // Compound assignment: the target is read as well as written, and the
        // index must still be evaluated exactly once, first.
        int[] c = new int[3];
        c[0] = 10;
        int i = 0;
        c[i++] += 5;
        sb.append(",").append(c[0]).append(",").append(i);

        // The place is a *field*, and the array access is a subexpression of it.
        EvalOrder[] arr = new EvalOrder[2];
        arr[0] = new EvalOrder();
        arr[1] = new EvalOrder();
        int j = 0;
        arr[j++].field = j;
        sb.append(",").append(arr[0].field);

        return sb.toString();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
