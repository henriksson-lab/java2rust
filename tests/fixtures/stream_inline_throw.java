import java.util.List;
import java.util.Set;
import java.util.stream.Collectors;

// Pending exception-prefix support, isolated from collection carrier selection.
class InlineThrowStreamSet {
    static <T> Set<T> checked(List<T> values, boolean fail) {
        return values.stream().map(value -> {
            if (fail) throw new IllegalArgumentException();
            return value;
        }).collect(Collectors.toSet());
    }
}
