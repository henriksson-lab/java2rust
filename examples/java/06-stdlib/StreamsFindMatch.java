package examples.stdlib;

import java.util.List;
import java.util.Optional;

public class StreamsFindMatch {
    public Optional<String> firstLong(List<String> values) {
        return values.stream()
                .filter(v -> v.length() > 3)
                .findFirst();
    }

    public boolean hasEmpty(List<String> values) {
        return values.stream().anyMatch(v -> v.isEmpty());
    }
}

