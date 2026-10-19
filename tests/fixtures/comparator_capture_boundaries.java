import java.util.Comparator;

interface KeyLookup<T> {
    int key(T value);
}

class MutableKeyLookup implements KeyLookup<Integer> {
    int scale;
    int calls;
    MutableKeyLookup(int scale) { this.scale = scale; }
    void setScale(int scale) { this.scale = scale; }
    public int key(Integer value) { calls++; return value / scale; }
}

class BorrowedKeyTouch {
    BorrowedKeyTouch(KeyLookup<Integer> source) { source.key(10); }
}

class ComparatorCaptureBoundaries {
    static <T> Comparator<T> order(KeyLookup<T> source) {
        return Comparator.comparingInt(source::key);
    }
    static <T> Comparator<T> forwardedOrder(KeyLookup<T> source) {
        return order(source);
    }
    static int read(KeyLookup<Integer> source) { return source.key(21); }
    static boolean present(KeyLookup<Integer> source) { return source != null; }
    static int reread(KeyLookup<Integer> source) {
        if (!present(source)) return -1;
        return read(source) + source.key(10);
    }
    static int borrowed() {
        MutableKeyLookup source = new MutableKeyLookup(10);
        return reread(source);
    }
    static int borrowedNull() { return reread(null); }
    static int constructorBorrow(KeyLookup<Integer> source) {
        new BorrowedKeyTouch(source);
        return source.key(21);
    }
    static int constructorBorrowed() {
        MutableKeyLookup source = new MutableKeyLookup(10);
        return constructorBorrow(source);
    }
    static int localNullable(KeyLookup<Integer> source) {
        if (source == null) return -1;
        int first = source.key(10);
        Comparator<Integer> comparator = Comparator.comparingInt(source::key);
        return first * 10 + comparator.compare(21, 10);
    }
    static int localNullableSample() {
        return localNullable(new MutableKeyLookup(10));
    }
    static int localNullableNull() { return localNullable(null); }
    static int nullBoundReference() {
        KeyLookup<Integer> source = null;
        try {
            Comparator<Integer> comparator = Comparator.comparingInt(source::key);
            return 0;
        } catch (NullPointerException error) { return 8; }
    }
    static int retained() {
        MutableKeyLookup source = new MutableKeyLookup(10);
        Comparator<Integer> comparator = forwardedOrder(source);
        source.setScale(100);
        return comparator.compare(21, 10);
    }
}
