import java.util.*;
import java.util.stream.Collectors;

interface OverloadedStreamContract<T> {
    boolean accepts(T value);
    boolean accepts(T left, T right);
}

class OverloadedStreamReceiver<T> implements OverloadedStreamContract<T> {
    public boolean accepts(T value) { return value != null; }
    public boolean accepts(T left, T right) { return left != null && right != null; }
}

class NullableStreamMembership {
    private Set<String> kept;
    NullableStreamMembership(Set<String> kept) { this.kept = kept; }
    Set<String> filter(List<String> values) {
        Set<String> result = values.stream().filter(kept::contains).collect(Collectors.toSet());
        Iterable<String> retained = result;
        return result;
    }
}

class RetainedStreamCallbackContracts {
    static <T> Set<T> filter(OverloadedStreamContract<T> receiver, List<T> values, T pivot) {
        Set<T> result = values.stream().filter(value -> receiver.accepts(value, pivot)).collect(Collectors.toSet());
        Iterable<T> retained = result;
        return result;
    }

    static int run() {
        List<String> words = Arrays.asList("a", "b");
        Set<String> selected = RetainedStreamCallbackContracts.<String>filter(
            new OverloadedStreamReceiver<String>(), words, "x");
        Set<String> kept = new HashSet<>();
        kept.add("x"); kept.add(null);
        List<String> values = new ArrayList<>();
        values.add("x"); values.add(null); values.add("q");
        Set<String> matches = new NullableStreamMembership(kept).filter(values);
        return selected.size() * 10 + matches.size();
    }
}
