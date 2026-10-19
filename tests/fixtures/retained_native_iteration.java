import java.util.*;

class RetainedNativeIteration {
    static int list() {
        List<Integer> values = new ArrayList<>();
        Iterable<Integer> alias = values;
        values.add(1); values.add(2);
        int result = 0;
        for (int value : values) { result += value; }
        return result;
    }
    static int set() {
        Set<Integer> values = new LinkedHashSet<>();
        Iterable<Integer> alias = values;
        values.add(2); values.add(3); values.add(2);
        int result = 0;
        for (int value : values) { result += value; }
        return result;
    }
    static void change(List<Integer> values) { values.add(9); }
    static int failFast() {
        List<Integer> values = new ArrayList<>();
        Iterable<Integer> alias = values;
        values.add(1); values.add(2);
        try { for (int value : values) { change(values); } }
        catch (ConcurrentModificationException failure) { return 7; }
        return -1;
    }
    static int nullSource() {
        List<Integer> values = Collections.emptyList();
        values = null;
        try { for (int value : values) { return -1; } }
        catch (NullPointerException failure) { return 5; }
        return -2;
    }
    static int fieldReplacement() {
        return new IterationFields().run();
    }
    static int arraySource() {
        List<Integer>[] rows = new List[1];
        List<Integer> values = new ArrayList<>();
        Iterable<Integer> alias = values;
        values.add(4);
        rows[0] = values;
        int result = 0;
        for (int value : rows[0]) { result += value; }
        return result;
    }
    static int nullElement() {
        List<Integer> values = new ArrayList<>();
        Iterable<Integer> alias = values;
        values.add(null);
        try { for (int value : values) { return value; } }
        catch (NullPointerException failure) { return 6; }
        return -1;
    }
    static int nullableElement() {
        List<Integer> values = new ArrayList<>();
        Iterable<Integer> alias = values;
        values.add(null);
        for (Integer value : values) { if (value == null) { return 7; } }
        return -1;
    }
    static int unhandledUnboxing() {
        List<Integer> values = new ArrayList<>();
        Iterable<Integer> alias = values;
        try {
            values.add(null);
            for (int value : values) { return value; }
        } catch (ConcurrentModificationException | NoSuchElementException | UnsupportedOperationException failure) {
            return -2;
        }
        return -1;
    }
    static int forwardedUnboxing() {
        try { return unhandledUnboxing(); }
        catch (NullPointerException failure) { return 8; }
    }
}

class IterationFields {
    List<Integer> values = new ArrayList<>();
    int run() {
        Iterable<Integer> alias = values;
        values.add(1); values.add(2);
        int result = 0;
        for (int value : values) {
            values = new ArrayList<>();
            values.add(9);
            result += value;
        }
        return result * 10 + values.size();
    }
}

class GenericIteration<T> {
    List<T> values = new ArrayList<>();
    void pin(List<List<T>> target) { target.add(values); }
    int forward() { return count(); }
    void add(T value) { values.add(value); }
    int countNonNull() {
        int result = 0;
        for (T value : values) { if (value != null) { result++; } }
        return result;
    }
    int count() {
        int result = 0;
        for (T value : values) { result++; }
        return result;
    }
}

class GenericRows<T> {
    List<List<T>> values = new ArrayList<>();
    void pin(List<List<List<T>>> target) { target.add(values); }
    int forward() { return count(); }
    int count() {
        int result = 0;
        for (List<T> row : values) { result += row.size(); }
        return result;
    }
}
