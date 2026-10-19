import java.util.function.Function;

interface Measure<T> { int measure(); }
class StringMeasure implements Measure<String> {
    public int measure() { return 4; }
}
class WildcardCallbackCarriers {
    static int invoke(Function<? super Measure<String>, Integer> callback, Measure<String> value) {
        return callback.apply(value);
    }
    static int probe() {
        Measure<String> value = new StringMeasure();
        return invoke(item -> item.measure(), value);
    }
}
