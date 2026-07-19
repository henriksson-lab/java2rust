package examples.stdlib;

import java.util.List;
import java.util.Objects;
import java.util.stream.Collectors;

public class StreamMapNullFilter {
    public List<String> lowerNonEmpty(List<String> values) {
        return values.stream().map(value -> {
            if (value.isEmpty()) {
                return null;
            }
            return value.toLowerCase();
        }).filter(Objects::nonNull).collect(Collectors.toList());
    }
}
