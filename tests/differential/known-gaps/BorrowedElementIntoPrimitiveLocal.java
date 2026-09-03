/**
 * Known gap: a borrowed collection element bound to a primitive local.
 *
 * Reading `list.get(i)` or a map entry's value yields the element *by reference*.
 * Contexts that keep it by reference are fine -- `String.valueOf(..)`,
 * concatenation, and comparison all work, and `NestedMapValueRead.java` gates
 * them. A primitive-value slot does not dereference it:
 *
 *     let second: i32 = m.get(k).expect(..).get(1)...;   // expected `i32`, found `&i32`
 *     sum = (sum).wrapping_add(e.1);                     // expected `i32`, found `&i32`
 *
 * Both are `E0308`. The two shapes below are the same defect reached two ways --
 * a declared local and a compound assignment -- so a fix that only handles one of
 * them leaves this fixture failing.
 *
 * A read-only entry-set loop is unaffected, which is why this went unnoticed: the
 * failure needs the value to land in a slot that owns a primitive.
 *
 * Java prints "2,3".
 *
 * See claude_audits/0059-concatenation-erases-reference-values-and-collections-lose-their-contract.md.
 */
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

public class BorrowedElementIntoPrimitiveLocal {
    public static String run() {
        List<Integer> values = new ArrayList<>(Arrays.asList(1, 2));
        int second = values.get(1); // declared primitive local

        Map<String, Integer> counts = new LinkedHashMap<>();
        counts.put("a", 1);
        counts.put("b", 2);
        int sum = 0;
        for (Map.Entry<String, Integer> entry : counts.entrySet()) {
            sum += entry.getValue(); // compound assignment
        }

        return second + "," + sum;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
