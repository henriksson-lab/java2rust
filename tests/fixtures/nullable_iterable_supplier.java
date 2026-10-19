import java.util.*;
import java.util.function.Supplier;
class NullableIterableSupplier {
    static <T> Supplier<Iterable<T>> factory(List<T> values) {
        Iterable<T> alias = values;
        values.add(null);
        return () -> values;
    }
    static int run() {
        List<String> values = new ArrayList<>();
        values.add("x");
        Supplier<Iterable<String>> supplier = factory(values);
        values.add("y");
        int result = 0;
        for (String value : supplier.get()) {
            result += value == null ? 100 : 1;
        }
        return result;
    }
}
