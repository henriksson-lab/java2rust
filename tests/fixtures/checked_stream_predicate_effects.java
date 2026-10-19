package sample.streams;

import java.util.List;
import java.util.ArrayList;

interface CheckedStreamPredicate<T> {
    boolean accepts(T value);
    boolean accepts(T left, T right);
}

class CheckedStreamPredicateImpl<T> implements CheckedStreamPredicate<T> {
    public boolean accepts(T value) { return false; }
    public boolean accepts(T left, T right) {
        if (right == null) throw new IllegalArgumentException();
        return left != null;
    }
}

class CheckedStreamEffects {
    static <T> boolean probe(List<T> values, CheckedStreamPredicate<T> predicate, T pivot) {
        return values.stream()
            .filter(value -> predicate.accepts(value, pivot))
            .anyMatch(value -> predicate.accepts(value, pivot) && !predicate.accepts(value));
    }
    static int run(boolean fail) {
        List<String> values = new ArrayList<>();
        values.add(null); values.add("a"); values.add("b");
        String pivot = "x";
        if (fail) pivot = null;
        try {
            boolean found = CheckedStreamEffects.<String>probe(values, new CheckedStreamPredicateImpl<String>(), pivot);
            return found ? 1 : 0;
        } catch (IllegalArgumentException ex) { return 7; }
    }
}
