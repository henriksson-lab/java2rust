import java.util.ArrayList;
import java.util.List;
import java.util.HashMap;
import java.util.Set;

interface FreshArrayView {
    int get();
    void set(int value);
}
class FreshArrayCell implements FreshArrayView {
    int value;
    FreshArrayCell(int value) { this.value = value; }
    public int get() { return value; }
    public void set(int value) { this.value = value; }
}
class FreshCollectionArrayProbe {
    List<FreshArrayCell> values = new ArrayList<>();
    int events;
    List<FreshArrayCell> receiver(boolean present) {
        events = events * 10 + 1;
        return present ? values : null;
    }
    int dimension(int size, boolean add) {
        events = events * 10 + 2;
        if (add) values.add(new FreshArrayCell(9));
        return size;
    }
    int badDimension() {
        events = events * 10 + 2;
        throw new IllegalStateException("dimension");
    }
    static int clearView(FreshArrayView[] values) {
        values[0] = null;
        return 23;
    }
    static int run() {
        List<String> plain = new ArrayList<>();
        plain.add("one");
        String[] words = plain.toArray(new String[4]);
        if (words.length != 4 || !words[0].equals("one") || words[1] != null || words[3] != null) return 13;
        String[] counted = plain.toArray(new String[plain.size()]);
        if (counted.length != 1 || !counted[0].equals("one")) return 16;
        HashMap<String, String> map = new HashMap<>();
        map.put("key", "value");
        map.put(null, "null-key");
        String[] keys = map.keySet().toArray(new String[4]);
        String[] vals = map.values().toArray(new String[3]);
        int nonNullKeys = 0;
        for (int i = 0; i < 2; i++) {
            if (keys[i] != null) {
                if (!keys[i].equals("key")) return 14;
                nonNullKeys++;
            }
        }
        if (keys.length != 4 || nonNullKeys != 1 || keys[2] != null || keys[3] != null) return 14;
        if (vals.length != 3 || vals[2] != null ||
            !(vals[0].equals("value") || vals[1].equals("value")) ||
            !(vals[0].equals("null-key") || vals[1].equals("null-key"))) return 15;
        HashMap<String, String> original = new HashMap<>();
        original.put("first", "one");
        Set<String> liveKeys = original.keySet();
        original.put("second", "two");
        HashMap<String, String> alias = original;
        original = new HashMap<>();
        original.put("replacement", "three");
        alias.put("third", "four");
        String[] live = liveKeys.toArray(new String[0]);
        if (live.length != 3) return 19;
        for (String key : live) if (key.equals("replacement")) return 20;
        List<double[]> vectors = new ArrayList<>();
        double[] vector = new double[] { 2 };
        vectors.add(vector);
        double[][] grid = vectors.toArray(new double[3][]);
        if (grid.length != 3 || grid[0] != vector || grid[1] != null || grid[2] != null) return 17;
        grid[0][0] = 5;
        if (vector[0] != 5) return 18;
        FreshCollectionArrayProbe owner = new FreshCollectionArrayProbe();
        FreshArrayCell[] empty = owner.values.toArray(new FreshArrayCell[3]);
        if (empty.length != 3 || empty[0] != null || empty[2] != null) return 1;
        FreshArrayCell cell = new FreshArrayCell(4);
        owner.values.add(cell);
        owner.values.add(null);
        FreshArrayCell[] small = owner.values.toArray(new FreshArrayCell[0]);
        FreshArrayCell[] exact = owner.values.toArray(new FreshArrayCell[2]);
        FreshArrayCell[] large = owner.values.toArray(new FreshArrayCell[5]);
        if (small.length != 2 || exact.length != 2 || large.length != 5) return 2;
        if (small[0] != cell || exact[1] != null || large[2] != null || large[4] != null) return 3;
        small[0].value = 7;
        if (cell.value != 7 || owner.values.get(0).value != 7) return 4;
        exact[0] = null;
        if (owner.values.get(0) != cell || small[0] != cell) return 5;
        owner.events = 0;
        FreshArrayCell[] grown = owner.receiver(true).toArray(new FreshArrayCell[owner.dimension(1, true)]);
        if (owner.events != 12 || grown.length != 3 || grown[2].value != 9) return 6;
        owner.events = 0;
        try { owner.receiver(false).toArray(new FreshArrayCell[owner.dimension(3, false)]); return 7; }
        catch (NullPointerException expected) { if (owner.events != 12) return 8; }
        owner.events = 0;
        try { owner.receiver(false).toArray(new FreshArrayCell[owner.dimension(-1, false)]); return 9; }
        catch (NegativeArraySizeException expected) { if (owner.events != 12) return 10; }
        owner.events = 0;
        try { owner.receiver(false).toArray(new FreshArrayCell[owner.badDimension()]); return 11; }
        catch (IllegalStateException expected) { if (owner.events != 12) return 12; }
        List<FreshArrayView> views = new ArrayList<>();
        views.add(cell);
        FreshArrayView[] viewArray = views.toArray(new FreshArrayView[0]);
        viewArray[0].set(clearView(viewArray));
        if (cell.value != 23 || viewArray[0] != null) return 21;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
