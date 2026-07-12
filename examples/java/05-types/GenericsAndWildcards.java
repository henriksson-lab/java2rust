package examples.types;

import java.util.List;

public class GenericsAndWildcards<T extends Number> {
    private final List<T> values;

    public GenericsAndWildcards(List<T> values) {
        this.values = values;
    }

    public T first() {
        return values.get(0);
    }

    public static int sizeOf(List<?> values) {
        return values.size();
    }
}

