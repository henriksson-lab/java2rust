import java.util.Arrays;
import java.util.HashSet;
import java.util.Set;

class FieldCallbacks {
    private Set<Integer> selected = new HashSet<Integer>();
    FieldCallbacks() { selected.add(2); selected.add(4); }
    long count(int[] values) { return Arrays.stream(values).filter(selected::contains).count(); }
    boolean any(int[] values) { return Arrays.stream(values).anyMatch(value -> this.selected.contains(value)); }
    static long probe() { return new FieldCallbacks().count(new int[] {1, 2, 4, 8}); }
    static boolean matches() { return new FieldCallbacks().any(new int[] {1, 4}); }
    static boolean misses() { return new FieldCallbacks().any(new int[] {1, 3}); }
}
