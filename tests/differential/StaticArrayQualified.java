/**
 * Static array element writes through a **qualified** owner.
 *
 * `claude_audits/done/0039` fixed `a[i] = v`, where the array is named bare. It
 * did not fix `Outer.a[i] = v`, which is the same write one syntax later — the
 * target still went through the by-value accessor, so it assigned into a
 * temporary and was **silently discarded**. Same defect, same silence, and the
 * first fix did not reach it because it matched only `Expr::Var`.
 *
 * The nested-class case matters separately: `Inner` writing `Outer.a[0]` has to
 * resolve the owner from the receiver rather than from the type being lowered,
 * which is the path a bare name cannot exercise.
 *
 * `Sizes.M` is a constant owned by a *different* type, which a per-owner fold
 * scope cannot see — it fell back to an empty `Vec`, so the array had length 0
 * and the first index panicked. That is the same failure as a dropped
 * initializer, reached by a different route.
 *
 * `N` is a `static final` used as the array length. That is ordinary Java and it
 * pins the other half: the initial-value fold has to see the owner's own
 * constants, or `new int[N]` falls back to an empty `Vec` and the first index
 * panics — which is exactly the declaration most likely to use a named size.
 *
 * Java prints "11,5,3,7,2,2,4,4".
 *
 * See claude_audits/done/0039-static-array-fields-lose-their-value-and-their-writes.md.
 */
public class StaticArrayQualified {
    static final int N = 3;
    static final int TWO = N - 1;

    static int[] a = new int[N];
    static int[] b = new int[TWO];
    static int[] c = new int[StaticArrayQualified.TWO];
    static int[] d = new int[Sizes.M];
    static int[] e = {N, Sizes.M};

    /** A constant owned by a *different* type: a per-owner scope cannot see it. */
    static class Sizes {
        static final int M = 4;
    }

    /** Writes an enclosing class's static array through a qualified name. */
    static class Inner {
        static void write() {
            StaticArrayQualified.a[0] = 11;
        }
    }

    public static String run() {
        StaticArrayQualified.b[0] = 7; // qualified, same class
        Inner.write(); // qualified, from a nested class
        a[1] = 5; // bare

        return a[0]
                + "," + a[1]
                + "," + a.length
                + "," + b[0]
                + "," + b.length
                + "," + c.length
                + "," + d.length
                + "," + e[1];
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
