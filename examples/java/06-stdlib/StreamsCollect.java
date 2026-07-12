package examples.stdlib;

import java.util.List;
import java.util.stream.Collectors;

public class StreamsCollect {
    public List<String> clean(List<String> values) {
        return values.stream()
                .filter(v -> !v.isEmpty())
                .map(v -> v.trim())
                .collect(Collectors.toList());
    }
}

