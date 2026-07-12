package examples.stdlib;

import java.util.List;

public class StreamCapturesLocal {
    public int addBase(List<Integer> values, int base) {
        return values.stream()
                .mapToInt(v -> v + base)
                .sum();
    }
}
