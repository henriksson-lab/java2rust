import java.util.Arrays;
import java.util.Iterator;
import java.util.function.Supplier;

class DeferredSequence<T> implements Iterable<T> {
    private Supplier<Iterable<T>> supplier;

    DeferredSequence(Supplier<Iterable<T>> supplier) { this.supplier = supplier; }

    public Iterator<T> iterator() { return supplier.get().iterator(); }
}

class LiveIterableSupplier {
    static int direct() {
        final int[] state = new int[] { 1 };
        DeferredSequence<Integer> view = new DeferredSequence<>(() -> Arrays.asList(state[0]));
        state[0] = 7;
        int total = 0;
        for (int value : view) { total += value; }
        state[0] = 9;
        for (int value : view) { total += value; }
        return total;
    }

    static int throughInterface() {
        final int[] state = new int[] { 1 };
        Iterable<Integer> view = new DeferredSequence<>(() -> Arrays.asList(state[0]));
        state[0] = 7;
        int total = 0;
        for (int value : view) { total += value; }
        state[0] = 9;
        for (int value : view) { total += value; }
        return total;
    }
}
