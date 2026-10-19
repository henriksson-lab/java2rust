import java.util.HashMap;
import java.util.Map;

public class MapEntryArraysOracle {
    public static void main(String[] arguments) {
        Map<String, String> values = new HashMap<>();
        values.put("key", "value");
        Object[] entries = MapEntryArrays.erased(values);
        values.clear();
        assert entries.length == 1;
        Map.Entry<?, ?> entry = (Map.Entry<?, ?>) entries[0];
        assert entry.getKey().equals("key") && entry.getValue().equals("value");
        System.out.println("map-entry-arrays: ok");
    }
}
