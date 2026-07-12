package examples.stdlib;

import java.util.List;

public class StreamsUse {
    public int positiveSum(List<Integer> values) {
        return values.stream()
                .filter(v -> v > 0)
                .mapToInt(v -> v)
                .sum();
    }
}

