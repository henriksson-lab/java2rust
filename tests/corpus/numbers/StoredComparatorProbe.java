import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.List;

public final class StoredComparatorProbe {
    static int comparisons;
    static final class Item {
        final int n;
        final int tag;
        Item(int n, int tag) { this.n = n; this.tag = tag; }
    }
    static final Comparator<Item> ORDER = (a,b) -> {
        comparisons++;
        return Integer.compare(a.n,b.n);
    };
    static void sort(List<Item> items, Comparator<Item> comparator) {
        Collections.sort(items,comparator);
    }
    public static int run() {
        List<Item> items = new ArrayList<>();
        sort(items,null);
        Item first = new Item(2,10);
        items.add(first);
        sort(items,null);
        if (items.get(0) != first || comparisons != 0) return 1;
        items.add(new Item(1,20));
        try { sort(items,null); return 2; }
        catch (ClassCastException expected) { }
        Collections.sort(items,ORDER);
        if (items.get(0).n != 1 || items.get(1) != first || comparisons == 0) return 3;
        items.add(new Item(2,30));
        sort(items,ORDER);
        if (items.get(1).tag != 10 || items.get(2).tag != 30) return 4;
        Comparator<Item> retained = ORDER;
        Collections.sort(items,retained);
        if (items.get(1) != first || items.get(2).tag != 30) return 5;
        List<Integer> numbers = new ArrayList<>();
        numbers.add(3); numbers.add(1); numbers.add(2);
        Collections.sort(numbers,null);
        if (numbers.get(0) != 1 || numbers.get(1) != 2 || numbers.get(2) != 3) return 6;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
