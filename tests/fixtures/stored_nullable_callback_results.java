import java.util.*;
import java.util.function.*;
class NullableCallbackRows<T> {
    Supplier<Iterable<T>> source;
    NullableCallbackRows(Supplier<Iterable<T>> source) { this.source = source; }
    Iterable<T> get() { return source.get(); }
}
class StoredNullableCallbackResults {
    static <T> NullableCallbackRows<T> construct(List<T> values) {
        return new NullableCallbackRows<T>(() -> values);
    }
    static <T> Supplier<Iterable<T>> local(List<T> values) {
        Supplier<Iterable<T>> result = () -> values;
        return result;
    }
    static <T> Supplier<Iterable<T>> assigned(List<T> values) {
        Supplier<Iterable<T>> result = null;
        result = () -> values;
        return result;
    }
    static <T> NullableCallbackRows<T> forwarded(List<T> values) {
        return wrap(() -> values);
    }
    static <T> NullableCallbackRows<T> stored(List<T> values) {
        NullableCallbackRows<T> rows = new NullableCallbackRows<T>(null);
        rows.source = () -> values;
        return rows;
    }
    static <T> Function<String, Iterable<T>> function(List<T> values) {
        Function<String, Iterable<T>> result = ignored -> values;
        return result;
    }
    static <T> NullableCallbackRows<T> wrap(Supplier<Iterable<T>> source) {
        return new NullableCallbackRows<T>(source);
    }
    static int count(Iterable<String> rows) {
        int result = 0;
        for (String value : rows) result += value == null ? 100 : 1;
        return result;
    }
    static int run() {
        List<String> values = new ArrayList<>();
        values.add(null);
        values.add("x");
        NullableCallbackRows<String> first = construct(values);
        Supplier<Iterable<String>> second = local(values);
        Supplier<Iterable<String>> third = assigned(values);
        NullableCallbackRows<String> fourth = forwarded(values);
        NullableCallbackRows<String> fifth = stored(values);
        Function<String, Iterable<String>> sixth = function(values);
        values.add("y");
        return count(first.get()) + count(second.get()) + count(third.get())
            + count(fourth.get()) + count(fifth.get()) + count(sixth.apply("ignored"));
    }
}
