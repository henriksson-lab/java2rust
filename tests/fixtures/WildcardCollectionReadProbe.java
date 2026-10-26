import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;

class WildcardReadValue {
    int value;
    WildcardReadValue(int value) { this.value = value; }
    int score() { return value; }
}

class WildcardReadOwner<T extends WildcardReadValue> {
    T anchor;
    WildcardReadOwner(T anchor) { this.anchor = anchor; }
    T head(List<? extends T> values) {
        Iterator<? extends T> cursor = values.iterator();
        return cursor.next();
    }
    T last(List<? extends T> values) { return values.get(values.size() - 1); }
    int localScore(List<? extends T> values) { T value = values.get(0); return value.score(); }
}

class WildcardCollectionReadProbe {
    static <T> T head(List<? extends T> values) {
        Iterator<? extends T> cursor = values.iterator();
        return cursor.next();
    }
    static <T> T last(List<? extends T> values) {
        return values.get(values.size() - 1);
    }
    static <T extends WildcardReadValue> T boundedHead(List<? extends T> values) {
        Iterator<? extends T> cursor = values.iterator();
        return cursor.next();
    }
    static <T extends WildcardReadValue> T boundedLast(List<? extends T> values) {
        return values.get(values.size() - 1);
    }
    static <T extends WildcardReadValue> int localScore(List<? extends T> values) {
        T value = values.get(0);
        return value.score();
    }
    static <T> T localLast(List<? extends T> values) {
        T value = values.get(values.size() - 1);
        return value;
    }
    static int run() {
        List<String> values = new ArrayList<>();
        values.add("first");
        values.add("last");
        if (!head(values).equals("first")) return 1;
        if (!last(values).equals("last")) return 2;
        List<WildcardReadValue> bounded = new ArrayList<>();
        bounded.add(new WildcardReadValue(7));
        bounded.add(new WildcardReadValue(9));
        if (boundedHead(bounded).score() != 7) return 3;
        if (boundedLast(bounded).score() != 9) return 4;
        WildcardReadOwner<WildcardReadValue> owner = new WildcardReadOwner<>(bounded.get(0));
        if (owner.head(bounded).score() != 7) return 5;
        if (owner.last(bounded).score() != 9) return 6;
        if (localLast(values) != values.get(1)) return 7;
        if (localScore(bounded) != 7 || owner.localScore(bounded) != 7) return 8;
        bounded.add(null);
        if (boundedLast(bounded) != null || localLast(bounded) != null) return 9;
        List<WildcardReadValue> absent = new ArrayList<>();
        absent.add(null);
        try { localScore(absent); return 10; }
        catch (NullPointerException expected) {}
        try { owner.localScore(absent); return 11; }
        catch (NullPointerException expected) {}
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
