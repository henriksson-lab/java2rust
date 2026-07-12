package examples.types;

import java.util.List;

public class WildcardBounds {
    public int readExtends(List<? extends Number> values) {
        return values.get(0).intValue();
    }

    public void writeSuper(List<? super Integer> values) {
        values.add(3);
    }
}

