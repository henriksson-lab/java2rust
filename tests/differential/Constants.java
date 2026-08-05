/**
 * JDK constants must carry their real values.
 *
 * `0`/`0.0`/`false` are the worst possible placeholders for a missing constant,
 * because they are *plausible*: they type-check, they survive review, and they
 * change behaviour rather than stopping it. `Math.PI == 0` silently corrupts
 * every trigonometric result; `Double.NEGATIVE_INFINITY == 0.0` inverts the
 * control flow of a sentinel loop; `Double.NaN == 0.0` turns an always-false
 * comparison into a sometimes-true one.
 *
 * Java prints "3,-3,true,true,true,true,true,-3,true,true".
 *
 * See claude_audits/done/0031-stubbed-constant-census.md.
 */
public class Constants {
    public static String run() {
        int best = Integer.MAX_VALUE;
        for (int x : new int[]{5, 3, 9}) { if (x < best) best = x; }
        int worst = Integer.MIN_VALUE;
        for (int x : new int[]{-5, -3}) { if (x > worst) worst = x; }

        boolean piOk = Math.PI > 3.14 && Math.PI < 3.15;
        boolean eOk = Math.E > 2.71 && Math.E < 2.72;

        // Smallest *positive* value, not the most negative one.
        boolean minPositive = Double.MIN_VALUE > 0;

        // The infinities have to out-rank the finite extremes, or saturation
        // guards stop saturating.
        boolean infOrdered = Double.POSITIVE_INFINITY > Double.MAX_VALUE
                && Double.NEGATIVE_INFINITY < -Double.MAX_VALUE;

        // NaN never equals itself. As `0.0` this flips to true whenever the
        // value under test happens to be zero.
        boolean nanNeverEqualsItself = Double.NaN != Double.NaN;
        boolean floatNanNeverEqualsItself = Float.NaN != Float.NaN;

        // The sentinel idiom the audit calls out: seeded with -inf so any real
        // value wins. Seeded with 0.0, every negative input loses.
        double maxSeen = Double.NEGATIVE_INFINITY;
        for (double x : new double[]{-9.0, -3.0, -7.0}) { if (x > maxSeen) maxSeen = x; }

        boolean floatInfOrdered = Float.POSITIVE_INFINITY > Float.MAX_VALUE;
        boolean longBounds = Long.MAX_VALUE > 0 && Long.MIN_VALUE < 0;

        return best
                + "," + worst
                + "," + piOk
                + "," + eOk
                + "," + minPositive
                + "," + infOrdered
                + "," + nanNeverEqualsItself
                + "," + (int) maxSeen
                + "," + floatNanNeverEqualsItself
                + "," + (floatInfOrdered && longBounds);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
