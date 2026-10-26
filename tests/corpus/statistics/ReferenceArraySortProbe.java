import java.util.Arrays;
import java.util.Comparator;

public final class ReferenceArraySortProbe {
    static final class Item {
        int key;
        final int id;
        Item(int key, int id) { this.key = key; this.id = id; }
    }
    static int order(Item a, Item b) {
        if (a == null) return b == null ? 0 : -1;
        if (b == null) return 1;
        return Integer.compare(a.key, b.key);
    }
    public static int run() {
        Item a = new Item(2, 1);
        Item b = new Item(1, 2);
        Item c = new Item(2, 3);
        Item[] values = {a, b, c, null};
        Item[] alias = values;
        Arrays.sort(values, (x, y) -> order(x, y));
        if (alias[0] != null || alias[1] != b || alias[2] != a || alias[3] != c) return 1;
        a.key = 0;
        Comparator<Item> stored = (x, y) -> order(x, y);
        Arrays.sort(alias, stored);
        if (values[0] != null || values[1] != a || values[2] != b || values[3] != c) return 2;
        Arrays.parallelSort(values, stored);
        if (alias[0] != null || alias[1] != a || alias[2] != b || alias[3] != c) return 6;
        Item[] single = {a};
        Arrays.sort(single, (Comparator<Item>) null);
        try { Arrays.sort(new Item[]{a, b}, (Comparator<Item>) null); return 3; }
        catch (ClassCastException expected) { }
        try { Arrays.sort(values, (x,y) -> { throw new IllegalArgumentException("compare"); }); return 4; }
        catch (IllegalArgumentException expected) {
            if (!"compare".equals(expected.getMessage())) return 5;
        }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
