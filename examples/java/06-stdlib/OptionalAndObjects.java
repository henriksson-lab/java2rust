package examples.stdlib;

import java.util.Objects;
import java.util.Optional;

public class OptionalAndObjects {
    public boolean same(String left, String right) {
        return Objects.equals(left, right);
    }

    public String fallback(Optional<String> value, String fallback) {
        return value.orElse(fallback);
    }
}

