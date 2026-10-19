import java.util.function.IntSupplier;
import java.util.function.BiFunction;

class InlineCounter implements IntSupplier {
    int value;
    InlineCounter(int initial) { value = initial; }
    public int getAsInt() { return value++; }
}

class InlineNativeValues {
    static int evaluations;
    static int invoke(IntSupplier callback) { return callback.getAsInt(); }
    static int maybe(IntSupplier callback, boolean use) {
        if (use) return callback.getAsInt();
        return 9;
    }
    static int apply(BiFunction<Integer, Integer, Integer> callback, int a, int b) {
        return callback.apply(a, b);
    }
    static int shared() {
        InlineCounter source = new InlineCounter(3);
        IntSupplier facade = source;
        int first = invoke(facade);
        int second = invoke(source);
        return first * 100 + second * 10 + source.getAsInt();
    }
    static int twoInputs() {
        BiFunction<Integer, Integer, Integer> callback = (a, b) -> a + b;
        return apply(callback, 20, 22);
    }
    static int caught() {
        IntSupplier callback = () -> { throw new IllegalArgumentException("failure"); };
        try { return invoke(callback); }
        catch (IllegalArgumentException error) { return 7; }
    }
    static int ignoredNull() { return maybe(null, false); }
    static int caughtNull() {
        try { return maybe(null, true); }
        catch (NullPointerException error) { return 8; }
    }
    static IntSupplier once() {
        evaluations++;
        return () -> 1;
    }
    static int evaluatedOnce() {
        evaluations = 0;
        int result = maybe(once(), false);
        return result + evaluations * 10;
    }
}
