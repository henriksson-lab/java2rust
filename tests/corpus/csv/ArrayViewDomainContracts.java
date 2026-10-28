import java.util.Arrays;
import java.util.List;

public class ArrayViewDomainContracts {
    static String[] data;
    static Iterable<String> kept;
    static Iterable<String> view() { return Arrays.asList(data); }
    static synchronized int consume(Iterable<String> values) {
        int result = 0;
        for (String value : values) {
            if ("first".equals(value)) data[1] = "changed";
            if ("changed".equals(value)) result += 1;
            if (value == null) result += 2;
            if ("\ud800".equals(value)) result += 4;
        }
        return result;
    }
    static Iterable<String[]> rows(String[][] source) { return Arrays.asList(source); }
    static void keep(String[] source) { kept = Arrays.asList(source); }
    static int run() {
        data = new String[]{"first", "old", null, "\ud800"};
        Iterable<String> local = view();
        if (consume(local) != 7) return 1;
        List<String> list = (List<String>) local;
        list.set(0, "through");
        if (!"through".equals(data[0])) return 2;
        String[] source = new String[]{"before"};
        keep(source);
        source[0] = "late";
        if (!"late".equals(kept.iterator().next())) return 3;
        if (!"late".equals(ArrayViewDomainContracts.kept.iterator().next())) return 5;
        source[0] = null;
        if ("late".equals(kept.iterator().next())) return 6;
        if (ArrayViewDomainContracts.kept.iterator().next() != null) return 7;
        String[] row = new String[]{"row"};
        String[][] matrix = new String[][]{row};
        Iterable<String[]> rows = rows(matrix);
        row[0] = "late";
        String[] selected = rows.iterator().next();
        if (selected != row || !"late".equals(selected[0])) return 4;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
