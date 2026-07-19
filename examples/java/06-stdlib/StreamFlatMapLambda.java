package examples.stdlib;

import java.util.List;
import java.util.stream.Collectors;

public class StreamFlatMapLambda {
    public List<String> flatten(List<List<String>> groups) {
        return groups.stream()
                .flatMap(group -> group.stream())
                .collect(Collectors.toList());
    }
}
