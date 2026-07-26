package examples.stdlib;

import java.util.List;
import java.util.stream.Collectors;

public class StreamStaticMethodReferences {
    public List<Integer> parseInts(List<String> values) {
        return values.stream()
                .map(Integer::parseInt)
                .collect(Collectors.toList());
    }

    public List<Double> parseDoubles(List<String> values) {
        return values.stream()
                .map(Double::parseDouble)
                .collect(Collectors.toList());
    }
}
