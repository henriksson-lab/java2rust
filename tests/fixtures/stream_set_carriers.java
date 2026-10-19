import java.util.List;
import java.util.Set;
import java.util.stream.Collectors;

class StreamSetCarriers {
    static <T> Set<T> copied(List<T> values) {
        return values.stream().collect(Collectors.toSet());
    }
    static <T> Set<T> filtered(List<T> values) {
        return values.stream().filter(value -> true).collect(Collectors.toSet());
    }
    static <T> Set<T> mapped(List<T> values) {
        return values.stream().map(value -> value).collect(Collectors.toSet());
    }
    static <T> Set<T> checked(List<T> values, boolean fail) {
        return values.stream().map(value -> checkedValue(value, fail)).collect(Collectors.toSet());
    }
    static <T> T checkedValue(T value, boolean fail) {
        if (fail) throw new IllegalArgumentException();
        return value;
    }
}
