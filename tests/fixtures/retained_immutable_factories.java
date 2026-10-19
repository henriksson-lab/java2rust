import java.util.*;

class RetainedImmutableFactories {
    static <T> Set<T> emptySet() { return Collections.emptySet(); }
    static <T> Set<T> oneSet(T value) { return Collections.singleton(value); }
    static <T> List<T> emptyList() { return Collections.emptyList(); }
    static <T> List<T> oneList(T value) { return Collections.singletonList(value); }
    static void add(Set<Integer> values) { values.add(8); }
    static List<List<Integer>> wrap(List<Integer> row) { return Collections.singletonList(row); }
    static <T> List<List<T>> reuse(List<T> row) {
        List<List<T>> result = Collections.singletonList(row);
        row.clear();
        return result;
    }

    static int genericReuse() {
        List<Integer> row = new ArrayList<>();
        row.add(1);
        List<List<Integer>> wrapped = RetainedImmutableFactories.<Integer>reuse(row);
        row.add(2);
        return wrapped.get(0).size() * 10 + row.size();
    }

    static int checked(List<Integer> values) {
        Iterable<Integer> view = values;
        return values.get(9);
    }

    static int mutationHandled(List<Integer> values) {
        Iterable<Integer> view = values;
        try { values.set(9, 1); }
        catch (UnsupportedOperationException failure) { return -1; }
        return 0;
    }

    static int effects() {
        List<Integer> values = new ArrayList<>();
        values.add(7);
        int score = 0;
        try { checked(values); }
        catch (IndexOutOfBoundsException failure) { score += 1; }
        try { mutationHandled(values); }
        catch (IndexOutOfBoundsException failure) { score += 10; }
        return score;
    }

    static int nested() {
        List<Integer> row = new ArrayList<>();
        row.add(1);
        List<List<Integer>> outer = wrap(row);
        List<List<Integer>> direct = Collections.singletonList(row);
        Iterable<List<Integer>> view = outer;
        row.add(2);
        List<Integer> recovered = outer.get(0);
        recovered.add(3);
        for (List<Integer> fromView : view) { fromView.add(4); }
        if (direct.get(0).size() != 4) { return -1; }
        return row.size() * 10 + outer.get(0).size();
    }

    static int nullable() {
        Set<Integer> single = Collections.singleton(null);
        Iterable<Integer> view = single;
        return single.contains(null) && single.size() == 1 ? 1 : -1;
    }

    static int unique() {
        Set<Integer> value = Collections.singleton(1);
        if (value.remove(2)) { return -1; }
        try { value.add(1); }
        catch (UnsupportedOperationException failure) { return value.size(); }
        return -2;
    }

    static int sets() {
        Set<Integer> empty = RetainedImmutableFactories.<Integer>emptySet();
        Iterable<Integer> emptyView = empty;
        int score = 0;
        empty.clear();
        if (empty.remove(99)) { return -1; }
        List<Integer> noValues = new ArrayList<>();
        if (empty.addAll(noValues)) { return -2; }
        try { emptyView.iterator().remove(); return -3; }
        catch (IllegalStateException failure) { }
        try { add(empty); }
        catch (UnsupportedOperationException failure) { score += 1; }
        Set<Integer> single = RetainedImmutableFactories.<Integer>oneSet(7);
        Iterable<Integer> singleView = single;
        if (single.contains(7) && !single.contains(9)) { score += 10; }
        if (!single.remove(9)) { score += 100; }
        try { single.remove(7); }
        catch (UnsupportedOperationException failure) { score += 1000; }
        try { single.add(7); }
        catch (UnsupportedOperationException failure) { score += 10000; }
        Iterator<Integer> cursor = singleView.iterator();
        cursor.next();
        try { cursor.remove(); }
        catch (UnsupportedOperationException failure) { score += 100000; }
        return score;
    }

    static int lists() {
        List<Integer> empty = RetainedImmutableFactories.<Integer>emptyList();
        Iterable<Integer> emptyView = empty;
        empty.clear();
        try { emptyView.iterator().remove(); return -2; }
        catch (IllegalStateException failure) { }
        int score = 0;
        if (empty.remove(Integer.valueOf(9))) { return -1; }
        try { empty.get(0); }
        catch (IndexOutOfBoundsException failure) { score += 1; }
        try { empty.set(0, 8); }
        catch (UnsupportedOperationException failure) { score += 10; }
        try { empty.add(8); }
        catch (UnsupportedOperationException failure) { score += 100; }
        List<Integer> single = RetainedImmutableFactories.<Integer>oneList(7);
        Iterable<Integer> singleView = single;
        if (single.contains(7)) { score += 1000; }
        if (!single.remove(Integer.valueOf(9))) { score += 10000; }
        try { single.set(0, 8); }
        catch (UnsupportedOperationException failure) { score += 100000; }
        Iterator<Integer> cursor = singleView.iterator();
        cursor.next();
        try { cursor.remove(); }
        catch (UnsupportedOperationException failure) { score += 1000000; }
        return score;
    }
}
