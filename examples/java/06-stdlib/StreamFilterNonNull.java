package examples.stdlib;

import java.util.List;
import java.util.Objects;
import java.util.stream.Collectors;

public class StreamFilterNonNull {
    public List<String> keepPresent(List<String> values) {
        return values.stream()
                .filter(Objects::nonNull)
                .collect(Collectors.toList());
    }
}
