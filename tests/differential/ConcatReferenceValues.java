/**
 * Java string concatenation converts every operand with String.valueOf, which
 * calls toString() on a reference. This fixture checks common concrete
 * reference shapes: boxed primitive, enum, user class, List element, and Map
 * element.
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
