import java.util.*;
import java.util.function.Supplier;
class DeferredRows<Element> {
    Supplier<Iterable<Element>> supplier;
    DeferredRows() { this(null); }
    DeferredRows(Supplier<Iterable<Element>> supplier) {
        this.supplier = Objects.requireNonNull(supplier);
    }
    Iterable<Element> get() { return supplier.get(); }
}
class NullableSamConstructor {
    static <T> DeferredRows<T> from(List<T> values) {
        return new DeferredRows<T>(() -> values);
    }
    static int run() {
        List<String> values = new ArrayList<>();
        DeferredRows<String> rows = from(values);
        values.add("x");
        int count = 0;
        for (String value : rows.get()) count++;
        return count;
    }
}
