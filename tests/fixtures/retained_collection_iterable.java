import java.util.ArrayList;
import java.util.Arrays;
import java.util.ConcurrentModificationException;
import java.util.Iterator;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.function.Supplier;

class RetainedCollectionSequence<T> implements Iterable<T> {
    private Supplier<Iterable<T>> supplier;
    RetainedCollectionSequence(Supplier<Iterable<T>> supplier) { this.supplier = supplier; }
    public Iterator<T> iterator() { return supplier.get().iterator(); }
}

class RetainedCollectionIterable {
    static int sum(Iterable<Integer> values) {
        int result = 0;
        for (int value : values) { result += value; }
        return result;
    }

    static int listAlias() {
        List<Integer> values = new ArrayList<>();
        values.add(1);
        Iterable<Integer> view = values;
        values.set(0, 7);
        values.add(2);
        int first = sum(view);
        values.set(0, 9);
        return first + sum(view);
    }

    static int setAlias() {
        LinkedHashSet<Integer> values = new LinkedHashSet<>();
        values.add(1);
        Iterable<Integer> view = values;
        values.add(3);
        int first = sum(view);
        values.remove(1);
        return first + sum(view);
    }

    static int supplierAlias() {
        List<Integer> values = new ArrayList<>();
        values.add(1);
        Iterable<Integer> view = new RetainedCollectionSequence<>(() -> values);
        values.set(0, 7);
        int first = sum(view);
        values.add(9);
        return first + sum(view);
    }

    static int iteratorRemoval() {
        List<Integer> values = new ArrayList<>();
        values.add(3);
        values.add(4);
        Iterable<Integer> view = values;
        Iterator<Integer> cursor = view.iterator();
        cursor.next();
        cursor.remove();
        return values.size() * 10 + cursor.next();
    }

    static int iteratorFailFast() {
        List<Integer> values = new ArrayList<>();
        values.add(1);
        Iterable<Integer> view = values;
        Iterator<Integer> cursor = view.iterator();
        values.add(2);
        try {
            cursor.next();
            return 0;
        } catch (ConcurrentModificationException error) { return -7; }
    }

    static int element(Iterable<Integer> values, int index) {
        if (values instanceof List) { return ((List<Integer>) values).get(index); }
        Iterator<Integer> cursor = values.iterator();
        for (int i = 0; i < index; i++) { cursor.next(); }
        return cursor.next();
    }

    static int listDowncast() { return element(Arrays.asList(3, 4), 1); }

    static int nonListBranch() {
        LinkedHashSet<Integer> values = new LinkedHashSet<>();
        values.add(5);
        values.add(6);
        return element(values, 1);
    }
}
