import java.util.*;

class GenericStreamValues<T> {
    List<T> values = new ArrayList<>();
    GenericStreamValues() { Iterable<T> alias = values; }
    void add(T value) { values.add(value); }
    int copyNonNull(List<T> target) {
        values.stream().filter(value -> value != null)
            .forEach(value -> target.add((T) value));
        return target.size();
    }
    int copyAll(List<T> target) {
        values.stream().forEach(value -> target.add((T) value));
        return target.size();
    }
    int copyAllReference(List<T> target) {
        values.stream().forEach(target::add);
        return target.size();
    }
    long nullCount() { return values.stream().filter(value -> null == value).count(); }
    long nonNullCount() { return values.stream().filter(value -> value != null).count(); }
}

class GenericStreamProbe {
    static <T> GenericStreamValues<T> fresh() { return new GenericStreamValues<T>(); }
    static int run() {
        GenericStreamValues<String> values = GenericStreamProbe.<String>fresh();
        values.add(null); values.add("x");
        List<String> nonNull = new ArrayList<>();
        List<String> all = new ArrayList<>();
        List<String> viaReference = new ArrayList<>();
        if (values.copyAllReference(viaReference) != 2) return -1;
        return values.copyNonNull(nonNull) * 1000
            + (int) values.nullCount() * 100
            + (int) values.nonNullCount() * 10
            + values.copyAll(all);
    }
}
