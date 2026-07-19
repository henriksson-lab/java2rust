package idiom.gaps;

import java.util.Map;

public class ReusedMapArgumentClone {
    Map<String, Integer> update(Map<String, Integer> values, String key) {
        values = patch(values, key);
        values = patch(values, key);
        return values;
    }

    Map<String, Integer> patch(Map<String, Integer> values, String key) {
        return values;
    }
}
