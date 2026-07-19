package examples.stdlib;

import java.util.List;
import java.util.stream.Collectors;

public class StreamBlockLambda {
    public List<String> normalize(List<String> values) {
        return values.stream()
                .map(value -> {
                    String trimmed = value.trim();
                    return trimmed.toLowerCase();
                })
                .collect(Collectors.toList());
    }
}
