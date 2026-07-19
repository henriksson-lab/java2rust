package examples.stdlib;

import java.util.ArrayList;
import java.util.List;

public class CollectionCapacityConstructor {
    private List<String> names;

    public List<String> copyHeaders() {
        List<String> headers = new ArrayList<>(names.size());
        return headers;
    }
}
