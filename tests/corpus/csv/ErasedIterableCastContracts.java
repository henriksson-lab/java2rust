import java.util.Arrays;
import java.util.Iterator;

public class ErasedIterableCastContracts {
    static String[] values;
    static int factories;
    static class Rows implements Iterable<String> {
        public Iterator<String> iterator() {
            factories++;
            return Arrays.asList(values).iterator();
        }
    }
    static Iterable<?> cast(Object source) { return (Iterable<?>) source; }
    static Object erase(Iterable<String> source) { return source; }
    static Object source(Rows input) { return input; }
    static synchronized int traverse(Iterable<?> source) {
        int result = 0;
        for (Object value : source) {
            if ("first".equals(value)) values[1] = "changed";
            if ("changed".equals(value)) result += 1;
            if (value == null) result += 2;
            if ("\ud800".equals(value)) result += 4;
        }
        return result;
    }
    static int run() {
        values = new String[]{"first", "old", null, "\ud800"};
        Object direct = new Rows();
        Rows root = new Rows();
        Object alias = source(root);
        if (source(null) != null || alias != root || !(alias instanceof Iterable)) return 11;
        if (direct.getClass() != Rows.class) return 12;
        if (!(direct instanceof Iterable) || factories != 0) return 1;
        Iterable<?> view = cast(direct);
        if (factories != 0) return 2;
        if (traverse(view) != 7 || factories != 1) return 3;
        values[1] = "old";
        Iterable<String> typed = new Rows();
        Object erased = erase(typed);
        if (erased != typed || factories != 1) return 4;
        if (!(erased instanceof Iterable) || traverse(cast(erased)) != 7) return 5;
        if (factories != 2) return 6;
        Iterator<?> iterator = cast(erased).iterator();
        values[0] = "late";
        if (!"late".equals(iterator.next())) return 7;
        if (cast(null) != null) return 8;
        try { cast(Integer.valueOf(3)); return 9; }
        catch (ClassCastException expected) { }
        try { traverse(cast(null)); return 10; }
        catch (NullPointerException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
