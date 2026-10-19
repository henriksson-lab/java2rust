import java.util.*;

interface ValueSink<T> {
    boolean put(T value);
    T first();
    int count();
}

class CountingSink<T> implements ValueSink<T> {
    private int size;
    public boolean put(T value) { size++; return true; }
    public T first() { return null; }
    public int count() { return size; }
}

class ExactBoundedSourceCalls {
    static <T> boolean addAll(ValueSink<? super T> destination, Collection<? extends T> values) {
        boolean modified = false;
        for (T value : values) modified |= destination.put(value);
        return modified;
    }

    static int run() {
        ValueSink<String> sink = new CountingSink<String>();
        List<String> values = Arrays.asList("a", "b");
        ExactBoundedSourceCalls.<String>addAll(sink, values);
        ValueSink<Integer> numbers = new CountingSink<Integer>();
        List<Integer> integers = Arrays.asList(1, 2, 3);
        ExactBoundedSourceCalls.<Integer>addAll(numbers, integers);
        return sink.count() * 10 + numbers.count();
    }
}
