package examples.stdlib;

import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;

public class CollectorsToMap {
    public Map<String, Integer> lengths(List<String> values) {
        return values.stream()
                .collect(Collectors.toMap(v -> v, v -> v.length()));
    }
}

