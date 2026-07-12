package examples.stdlib;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public class CollectionsUse {
    public Map<String, Integer> counts(List<String> names) {
        Map<String, Integer> counts = new HashMap<>();
        for (String name : names) {
            Integer old = counts.get(name);
            counts.put(name, old == null ? 1 : old + 1);
        }
        return counts;
    }

    public List<String> singleton(String value) {
        List<String> values = new ArrayList<>();
        values.add(value);
        return values;
    }
}

