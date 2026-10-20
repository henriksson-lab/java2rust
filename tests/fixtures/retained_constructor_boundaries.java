import java.util.Arrays;
import java.util.Collections;
import java.util.List;

class RetainedConstructorHolder<T> {
    List<T> items;
    RetainedConstructorHolder(List<T> values) { items = values; }
    RetainedConstructorHolder(T[] values) { this(Arrays.asList(values)); }
    RetainedConstructorHolder(int missing) { this((List<T>) null); }
    static <T> RetainedConstructorHolder<T> empty() {
        return new RetainedConstructorHolder<T>(Collections.emptyList());
    }
    static <T> RetainedConstructorHolder<T> one(T value) {
        return new RetainedConstructorHolder<T>(Collections.singletonList(value));
    }
}

class RetainedConstructorBoundaries {
    static int run() {
        String[] values = new String[] { "one" };
        RetainedConstructorHolder<String> view = new RetainedConstructorHolder<String>(values);
        view.items.set(0, "two");
        if (!values[0].equals("two")) return -1;
        RetainedConstructorHolder<String> one = RetainedConstructorHolder.one("x");
        RetainedConstructorHolder<String> empty = RetainedConstructorHolder.empty();
        int result = one.items.size();
        if (empty.items.size() != 0) return -2;
        try { one.items.add("no"); }
        catch (UnsupportedOperationException expected) { result += 2; }
        try { empty.items.add("no"); }
        catch (UnsupportedOperationException expected) { result += 4; }
        RetainedConstructorHolder<String> missing = new RetainedConstructorHolder<String>(0);
        if (missing.items != null) return -3;
        RetainedConstructorHolder<String> forwarded = new RetainedConstructorHolder<String>(view.items);
        forwarded.items.set(0, "three");
        if (!values[0].equals("three")) return -4;
        return result;
    }
}
