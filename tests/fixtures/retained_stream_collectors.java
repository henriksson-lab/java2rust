import java.util.*;
import java.util.stream.Collectors;

class RetainedStreamCollectors {
    static <T> Set<T> addNullToCopy(List<T> source) {
        Set<T> result = source.stream().collect(Collectors.toSet());
        Iterable<T> retained = result;
        result.add(null);
        return result;
    }

    static int listPolicies() {
        List<Integer> source = Arrays.asList(1, 2);
        List<Integer> linked = source.stream().collect(Collectors.toCollection(LinkedList::new));
        Iterable<Integer> linkedView = linked;
        Iterator<Integer> linkedCursor = linkedView.iterator();
        linked.addAll(Collections.emptyList());
        int score = linkedCursor.next();
        List<Integer> array = source.stream().collect(Collectors.toList());
        Iterable<Integer> arrayView = array;
        Iterator<Integer> arrayCursor = arrayView.iterator();
        array.addAll(Collections.emptyList());
        try { arrayCursor.next(); }
        catch (ConcurrentModificationException error) { score += 10; }
        return score;
    }

    static <T> Set<T> mapped(List<T> values, java.util.function.Function<T, T> mapper) {
        Set<T> result = values.stream().map(value -> mapper.apply(value)).collect(Collectors.toSet());
        Iterable<T> retained = result;
        return result;
    }

    static int checked() {
        List<Integer> values = Arrays.asList(1, 2, 3);
        try {
            mapped(values, value -> {
                if (value == 2) { throw new IllegalStateException(); }
                return value;
            });
        } catch (IllegalStateException error) { return values.size() + 4; }
        return -1;
    }

    static <T> Set<T> set(List<T> values) {
        Set<T> result = values.stream().collect(Collectors.toSet());
        Iterable<T> retained = result;
        return result;
    }

    static <T> Set<T> ordered(List<T> values) {
        Set<T> result = values.stream().collect(Collectors.toCollection(LinkedHashSet::new));
        Iterable<T> retained = result;
        return result;
    }

    static List<Integer> list(List<Integer> values) {
        List<Integer> result = values.stream().filter(value -> value > 1).collect(Collectors.toList());
        Iterable<Integer> retained = result;
        return result;
    }

    static int run() {
        List<Integer> source = new ArrayList<>();
        source.add(3); source.add(1); source.add(3); source.add(2);
        Set<Integer> result = ordered(source);
        Iterable<Integer> alias = result;
        result.add(4);
        int digits = 0;
        for (int value : alias) { digits = digits * 10 + value; }
        source.clear();
        List<Integer> filtered = list(Arrays.asList(1, 2, 3));
        Iterable<Integer> listAlias = filtered;
        filtered.set(0, 7);
        int sum = 0;
        for (int value : listAlias) { sum += value; }
        return digits + sum;
    }

    static int nullable() {
        List<String> values = new ArrayList<>();
        values.add(null); values.add("x"); values.add(null);
        Set<String> all = set(values);
        Set<String> nonNull = values.stream().filter(Objects::nonNull).collect(Collectors.toSet());
        Iterable<String> retained = nonNull;
        return all.size() * 10 + nonNull.size();
    }
}
