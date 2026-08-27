/**
 * Java's string concatenation converts every operand with `String.valueOf`,
 * which calls `toString()` on a reference. This fixture checks that the value
 * actually survives the conversion for the reference shapes that appear in
 * almost every program: a boxed primitive, an enum constant, an enum `name()`,
 * a user class with an overridden `toString()`, and an element read out of a
 * `List<Integer>`.
 *
 * Java prints
 *
 *     5|BLUE|BLUE|P9|42|s|7
 */
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public class ConcatReferenceValues {
    enum Col { RED, BLUE }

    static class P {
        final int x;
        P(int x) { this.x = x; }
        public String toString() { return "P" + x; }
    }

    public static String run() {
        Integer boxed = 5;
        Col c = Col.BLUE;
        P p = new P(9);
        List<Integer> li = new ArrayList<>();
        li.add(42);
        List<String> ls = new ArrayList<>();
        ls.add("s");
        Map<String, Integer> m = new HashMap<>();
        m.put("k", 7);
        return boxed + "|" + c + "|" + c.name() + "|" + p + "|" + li.get(0) + "|" + ls.get(0) + "|" + m.get("k");
    }

    public static void main(String[] args) { System.out.println(run()); }
}
