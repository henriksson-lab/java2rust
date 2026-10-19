import java.util.*;
import java.util.stream.*;

class RetainedStreams {
    static int matches() {
        Set<Integer> values = new LinkedHashSet<>();
        Iterable<Integer> alias = values;
        values.add(1); values.add(2);
        return (values.stream().allMatch(value -> value > 0) ? 10 : 0)
            + (values.stream().anyMatch(value -> value == 2) ? 1 : 0);
    }
    static int mapped() {
        List<Integer> values = new ArrayList<>();
        Iterable<Integer> alias = values;
        values.add(1); values.add(2); values.add(3);
        List<Integer> result = values.stream().filter(value -> value > 1)
            .map(value -> value + 10).collect(Collectors.toList());
        return result.get(0) + result.get(1);
    }
    static int forEach() {
        List<Integer> values = new ArrayList<>();
        Iterable<Integer> alias = values;
        values.add(1); values.add(2); values.add(3);
        List<Integer> target = new ArrayList<>();
        values.stream().forEach(value -> target.add(value));
        return target.size();
    }
    static int failFast() {
        List<Integer> values = new ArrayList<>();
        Iterable<Integer> alias = values;
        values.add(1); values.add(2);
        try { values.stream().forEach(value -> values.add(value + 9)); }
        catch (ConcurrentModificationException failure) { return 7; }
        return -1;
    }
    static int nestedReference() {
        List<List<Integer>> rows = new ArrayList<>();
        Iterable<List<Integer>> alias = rows;
        List<Integer> row = new ArrayList<>();
        row.add(1);
        rows.add(row);
        rows.stream().forEach(item -> item.add(2));
        return row.size();
    }
    static List<Integer> maybeValues(boolean missing) {
        if (missing) return null;
        List<Integer> values = new ArrayList<>();
        Iterable<Integer> alias = values;
        values.add(4);
        return values;
    }
    static int nullableSource(boolean missing) {
        try { return (int) maybeValues(missing).stream().count(); }
        catch (NullPointerException failure) { return 9; }
    }
    static int nestedSizes() {
        List<List<Integer>> rows = new ArrayList<>();
        List<Integer> row = new ArrayList<>();
        row.add(1); row.add(2);
        rows.add(row);
        return rows.stream().mapToInt(List::size).sum();
    }
    static int nullableParameter(List<Integer> values) {
        Iterable<Integer> alias = values;
        try {
            int first = (int) values.stream().count();
            return first + (int) values.stream().count();
        } catch (NullPointerException failure) { return 9; }
    }
    static int nullableParameterCall(boolean missing) {
        return nullableParameter(maybeValues(missing));
    }
}

class GenericRowStreams<T> {
    List<List<T>> values = new ArrayList<>();
    void pin(List<List<List<T>>> target) { target.add(values); }
    long count() { return values.stream().count(); }
}
