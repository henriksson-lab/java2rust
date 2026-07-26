package examples.stdlib;

import java.util.List;
import java.util.stream.Collectors;

public class StreamStringMethodReferences {
    public List<Integer> lengths(List<String> values) {
        return values.stream()
                .map(String::length)
                .collect(Collectors.toList());
    }

    public List<String> trimmed(List<String> values) {
        return values.stream()
                .map(String::trim)
                .collect(Collectors.toList());
    }

    public boolean anyEmpty(List<String> values) {
        return values.stream()
                .anyMatch(String::isEmpty);
    }
}
