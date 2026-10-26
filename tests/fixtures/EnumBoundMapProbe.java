import java.util.HashMap;
import java.util.Map;

enum BoundMapKey { FIRST, SECOND }
class EnumBoundCell<E extends Enum<E>> {
    E value;
    EnumBoundCell(E value) { this.value = value; }
}
enum BoundMapDisplay {
    LABEL, NULL_TEXT, FAIL;
    BoundMapDisplay same() { return this; }
    public String toString() {
        if (this == NULL_TEXT) return null;
        if (this == FAIL) throw new IllegalStateException("display");
        return "decorated:\uD800";
    }
}
class EnumBoundStore<E extends Enum<E>> {
    Map<E, Integer> values = new HashMap<>();
    E current;
    EnumBoundStore<E> same() { return this; }
    void setCurrent(E key) { current = key; }
    String currentText() { return current.toString(); }
    void put(E key, int value) { values.put(key, value); }
    int get(E key) { return values.get(key); }
    int getCell(EnumBoundCell<E> cell) { return values.get(cell.value); }
    boolean hasCell(EnumBoundCell<E> cell) { return values.containsKey(cell.value); }
    int removeCell(EnumBoundCell<E> cell) { return values.remove(cell.value); }
    String label(E key) { return key.name(); }
    String text(E key) { return key.toString(); }
    int order(E key) { return key.ordinal(); }
}
class EnumBoundMapProbe {
    static int run() {
        EnumBoundStore<BoundMapKey> store = new EnumBoundStore<>();
        store.put(BoundMapKey.FIRST, 7);
        store.put(BoundMapKey.SECOND, 9);
        if (store.get(BoundMapKey.FIRST) != 7) return 1;
        if (store.get(BoundMapKey.SECOND) != 9) return 2;
        if (!store.label(BoundMapKey.FIRST).equals("FIRST")) return 3;
        if (store.order(BoundMapKey.SECOND) != 1) return 4;
        if (store.label(BoundMapKey.FIRST) != BoundMapKey.FIRST.name()) return 5;
        if (!store.text(BoundMapKey.SECOND).equals("SECOND")) return 6;
        try { store.text(null); return 7; }
        catch (NullPointerException expected) {}
        EnumBoundStore<BoundMapDisplay> display = new EnumBoundStore<>();
        if (!display.text(BoundMapDisplay.LABEL).equals("decorated:\uD800")) return 8;
        if (display.text(BoundMapDisplay.NULL_TEXT) != null) return 9;
        try { display.text(BoundMapDisplay.FAIL); return 10; }
        catch (IllegalStateException expected) {}
        EnumBoundStore<BoundMapDisplay> alias = display.same();
        alias.setCurrent(BoundMapDisplay.LABEL);
        if (!display.currentText().equals("decorated:\uD800")) return 11;
        if (BoundMapDisplay.LABEL.same() != BoundMapDisplay.LABEL) return 12;
        EnumBoundCell<BoundMapKey> cell = new EnumBoundCell<>(BoundMapKey.FIRST);
        if (store.getCell(cell) != 7 || !store.hasCell(cell)) return 13;
        if (store.removeCell(cell) != 7 || store.hasCell(cell)) return 14;
        cell.value = null;
        if (store.hasCell(cell)) return 15;
        store.put(null, 11);
        if (store.getCell(cell) != 11 || !store.hasCell(cell)) return 16;
        if (store.removeCell(cell) != 11) return 17;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
