import java.util.*;
import java.util.function.*;

class CheckedDeferred<T> implements Iterable<T> {
    Supplier<Iterable<T>> source;
    CheckedDeferred(Supplier<Iterable<T>> source) {
        this.source = Objects.requireNonNull(source);
    }
    public Iterator<T> iterator() { return source.get().iterator(); }
}

interface DeferredView<T> {
    List<T> values();
    default Iterable<T> view() { return new CheckedDeferred<>(() -> values()); }
}

class MutableDeferredView implements DeferredView<Integer> {
    List<Integer> current;
    int calls;
    public List<Integer> values() { calls++; return current; }
}

class DeferredSourceViews {
    static Iterable<String> forwarded(CheckedDeferred<String> values) { return values; }
    static boolean missing(String value) { return value == null; }
    static int countMissing(Iterable<String> values) {
        int count = 0;
        for (String value : values) if (missing(value)) count++;
        return count;
    }
    static int nullableElements() {
        List<String> rows = new ArrayList<>();
        rows.add(null);
        rows.add("x");
        CheckedDeferred<String> wrapper = new CheckedDeferred<>(() -> rows);
        Iterator<String> cursor = wrapper.iterator();
        if (cursor.next() != null) return 1;
        if (!cursor.next().equals("x") || cursor.hasNext()) return 2;
        Iterable<String> proxy = wrapper;
        Iterator<String> second = proxy.iterator();
        if (second.next() != null || !second.next().equals("x") || second.hasNext()) return 3;
        if (countMissing(proxy) != 1 || countMissing(wrapper) != 1) return 4;
        int missing = 0;
        for (String value : wrapper) if (missing(value)) missing++;
        if (missing != 1) return 5;
        missing = 0;
        for (String value : forwarded(wrapper)) if (missing(value)) missing++;
        if (missing != 1) return 6;
        return 0;
    }
    static int sum(Iterable<Integer> values) {
        int result = 0;
        for (int value : values) result += value;
        return result;
    }
    static int run() {
        MutableDeferredView source = new MutableDeferredView();
        source.current = Arrays.asList(1, 2);
        DeferredView<Integer> proxy = source;
        Iterable<Integer> deferred = proxy.view();
        if (source.calls != 0) return 1;
        source.current = Arrays.asList(9);
        if (sum(deferred) != 9 || source.calls != 1) return 2;
        source.current = Arrays.asList(7, 8);
        if (sum(deferred) != 15 || source.calls != 2) return 3;
        source.current = null;
        try { sum(deferred); return 4; }
        catch (NullPointerException expected) { }
        if (source.calls != 3) return 5;
        CheckedDeferred<Integer> direct = new CheckedDeferred<>(() -> source.values());
        if (direct.source.get() != null || source.calls != 4) return 7;
        try { new CheckedDeferred<Integer>(null); return 6; }
        catch (NullPointerException expected) { }
        if (nullableElements() != 0) return 8;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
