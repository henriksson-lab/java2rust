package examples.types;

import java.util.ArrayList;
import java.util.List;

public class RawTypes {
    public Object firstRaw(List values) {
        values.add("text");
        values.add(1);
        return values.get(0);
    }

    public List makeRaw() {
        return new ArrayList();
    }
}

