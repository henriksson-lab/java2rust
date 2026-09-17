/**
 * Promoted regression for borrowed collection elements consumed by primitive
 * value slots. Java prints "2,3".
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
