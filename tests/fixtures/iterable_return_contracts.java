import java.util.*;
import java.util.function.Supplier;

class IterableReturnContracts<T> {
    List<T> values = new ArrayList<>();
    IterableReturnContracts() { Iterable<T> alias = values; }
    void add(T value) { values.add(value); }
    Iterable<T> direct() { return values; }
    Iterable<T> conditional(boolean first) { return first ? values : values; }
    static <T> Iterable<T> forward(IterableReturnContracts<T> source) {
        return source.direct();
    }
    static <T> int count(Iterable<T> values) {
        int count = 0;
        for (T value : values) count++;
        return count;
    }
    int argument() { return count(values); }
    IterableReturnBox<T> box() { return new IterableReturnBox<T>(values); }
    static <T> Supplier<Iterable<T>> deferred(List<T> values) { return () -> values; }
    static <T> Supplier<Iterable<T>> deferredBlock(List<T> values) {
        return () -> { List<T> alias = values; return alias; };
    }
}

class IterableReturnBox<T> {
    Iterable<T> values;
    IterableReturnBox(Iterable<T> values) { this.values = values; }
    int count() {
        int count = 0;
        for (T value : values) count++;
        return count;
    }
}

class IterableReturnProbe {
    static int run() {
        IterableReturnContracts<String> source = new IterableReturnContracts<>();
        source.add("x");
        Iterable<String> direct = source.direct();
        Iterable<String> conditional = source.conditional(false);
        Supplier<Iterable<String>> deferred = IterableReturnContracts.deferred(source.values);
        Supplier<Iterable<String>> deferredBlock = IterableReturnContracts.deferredBlock(source.values);
        source.add("y");
        int count = 0;
        for (String value : direct) count++;
        for (String value : conditional) count++;
        for (String value : IterableReturnContracts.forward(source)) count++;
        count += source.argument();
        count += source.box().count();
        for (String value : deferred.get()) count++;
        for (String value : deferredBlock.get()) count++;
        return count;
    }
}
