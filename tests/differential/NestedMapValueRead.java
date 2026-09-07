/**
 * A read that chains a method call onto a map value.
 *
 * `m.get("a")` yields an optional, and the projection that discharges it emits
 * `.expect("Java null receiver")`. The outer `.get(1)` then adapts its own
 * receiver and asks the *plan* whether that receiver is optional -- the plan still
 * says yes, because the Rust expression carries no record of what was already done
 * to it (0007/0008). So a second identical unwrap was appended, landing on the
 * `Vec` the first one produced:
 *
 *     m.get(k).expect("Java null receiver").expect("Java null receiver").get(1)
 *
 * which is `E0599`. Around 150 sites independently decide to discharge an optional
 * receiver, so "did someone already do this" needs one answer rather than a list of
 * the shapes anyone happened to see: the null-receiver unwrap is now idempotent at
 * its constructor, and the after-the-fact strip pass shares the same predicate
 * instead of carrying three hand-written shape lists of its own.
 *
 * The reads below go through contexts that hold the element by reference. Binding
 * the same read to an `int` local is covered separately by
 * `BorrowedElementIntoPrimitiveLocal.java`.
 *
 * Java prints "2,1,2".
 *
 * See claude_audits/0056-null-is-planned-away-not-represented.md.
 */
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public class NestedMapValueRead {
    public static String run() {
        Map<String, List<Integer>> m = new HashMap<>();
        m.put("a", new ArrayList<>(Arrays.asList(1, 2)));

        return String.valueOf(m.get("a").get(1))
                + "," + String.valueOf(m.get("a").get(0))
                + "," + m.get("a").size();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
