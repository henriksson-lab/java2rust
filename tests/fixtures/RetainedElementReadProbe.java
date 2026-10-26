import java.util.ArrayList;
import java.util.List;

class ElementReadItem {
    int value;
    ElementReadItem(int value) { this.value = value; }
    void add(int increment) { value += increment; }
    int read() { return value; }
}
class ElementReadStore<K> {
    List<K> values = new ArrayList<>();
    void add(K value) { values.add(value); }
    K get(int index) { return values.get(index); }
    K replace(int index, K value) { return values.set(index, value); }
    K remove(int index) { return values.remove(index); }
}
class RetainedElementReadProbe {
    static int run() {
        ElementReadItem item = new ElementReadItem(3);
        List<ElementReadItem> direct = new ArrayList<>();
        direct.add(item);
        direct.get(0).add(4);
        if (item.read() != 7 || direct.get(0) != item) return 1;
        ElementReadStore<ElementReadItem> store = new ElementReadStore<>();
        store.add(item);
        store.add(null);
        if (store.get(0) != item || store.get(1) != null) return 2;
        store.get(0).add(2);
        if (item.read() != 9) return 3;
        if (store.replace(0, null) != item || store.get(0) != null) return 4;
        if (store.remove(0) != null || store.remove(0) != null) return 5;
        try { direct.get(-1); return 6; }
        catch (IndexOutOfBoundsException expected) {}
        try { direct.get(1); return 7; }
        catch (IndexOutOfBoundsException expected) {}
        direct.add(null);
        try { direct.get(1).read(); return 8; }
        catch (NullPointerException expected) {}
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
