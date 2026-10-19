import java.util.Arrays;
import java.util.Iterator;

class FailingIterable implements Iterable<Integer> {
    public Iterator<Integer> iterator() { throw new IllegalStateException(); }
}

class MutableIterable implements Iterable<Integer> {
    int value;
    public Iterator<Integer> iterator() { return Arrays.asList(value).iterator(); }
}

class CheckedIterable {
    static int consume(Iterable<Integer> values) {
        try {
            int sum = 0;
            for (int value : values) { sum += value; }
            return sum;
        } catch (IllegalStateException error) { return -7; }
    }

    static int factoryFailure() { return consume(new FailingIterable()); }

    static int sourceView() {
        MutableIterable source = new MutableIterable();
        Iterable<Integer> view = source;
        source.value = 7;
        int sum = consume(view);
        source.value = 9;
        return sum + consume(view);
    }

    static int freshLists() {
        Iterable<Integer> values = Arrays.asList(3, 4);
        int sum = 0;
        for (int value : values) { sum += value; }
        for (int value : values) { sum += value; }
        return sum;
    }
}
