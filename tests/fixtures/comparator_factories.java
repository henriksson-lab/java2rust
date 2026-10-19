import java.util.Comparator;

class ComparatorKeySource {
    int scale;
    ComparatorKeySource(int scale) { this.scale = scale; }
    void setScale(int scale) { this.scale = scale; }
    int key(int value) { return value / scale; }
    int key(String value) { return 42; }
    int onlyKey(Integer value) { return value / scale; }
}

class ComparatorFactories {
    static int calls;
    static int key(int value) {
        calls++;
        if (value < 0) throw new IllegalArgumentException("negative key");
        return value / 10;
    }
    static Comparator<Integer> keyOrder() {
        return Comparator.comparingInt(ComparatorFactories::key);
    }
    static Comparator<Integer> captured(ComparatorKeySource source) {
        return Comparator.comparingInt(source::key);
    }
    static int count() { return calls; }
    static int stringKey(String value) { return value.length(); }
    static Comparator<String> stringOrder() {
        return Comparator.comparingInt(ComparatorFactories::stringKey);
    }
    static int stringCompared() { return stringOrder().compare("aa", "b"); }
    static int noEager() {
        calls = 0;
        keyOrder();
        return calls;
    }
    static int compared() {
        calls = 0;
        int result = keyOrder().compare(21, 10);
        return result * 10 + calls;
    }
    static int reversed() { return keyOrder().reversed().compare(21, 10); }
    static int chained() {
        Comparator<Integer> comparator = Comparator.comparingInt((Integer value) -> value / 10)
            .thenComparingInt(value -> value % 10);
        return comparator.compare(21, 22);
    }
    static int caught() {
        calls = 0;
        Comparator<Integer> comparator = keyOrder();
        try { return comparator.compare(-1, 10); }
        catch (IllegalArgumentException error) { return 9; }
    }
    static int retained() {
        ComparatorKeySource source = new ComparatorKeySource(10);
        Comparator<Integer> comparator = captured(source);
        source.setScale(100);
        return comparator.compare(21, 10);
    }
    static int caughtCapture() {
        ComparatorKeySource source = new ComparatorKeySource(10);
        Comparator<Integer> comparator = captured(source);
        source.setScale(0);
        try { return comparator.compare(21, 10); }
        catch (ArithmeticException error) { return 8; }
    }
    static int rebound() {
        ComparatorKeySource source = new ComparatorKeySource(10);
        Comparator<Integer> comparator = Comparator.comparingInt(source::key);
        source = new ComparatorKeySource(100);
        return comparator.compare(21, 10);
    }
    static int boundChain() {
        ComparatorKeySource source = new ComparatorKeySource(10);
        Comparator<Integer> comparator = Comparator.comparingInt(source::onlyKey)
            .thenComparingInt(value -> value % 10);
        source.setScale(100);
        return comparator.compare(21, 22);
    }
}
