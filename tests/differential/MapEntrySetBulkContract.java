import java.util.AbstractMap;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;

public class MapEntrySetBulkContract {
    public static String run() {
        Map<String, String> values = new HashMap<>();
        values.put("a", "1");
        values.put("b", "2");
        values.put("c", "3");

        Set<Map.Entry<String, String>> required = new HashSet<>();
        required.add(new AbstractMap.SimpleEntry<String, String>("a", "1"));
        required.add(new AbstractMap.SimpleEntry<String, String>("c", "3"));

        Set<Map.Entry<String, String>> keep = new HashSet<>();
        keep.add(new AbstractMap.SimpleEntry<String, String>("b", "2"));
        keep.add(new AbstractMap.SimpleEntry<String, String>("c", "3"));

        Set<Map.Entry<String, String>> drop = new HashSet<>();
        drop.add(new AbstractMap.SimpleEntry<String, String>("b", "2"));
        drop.add(new AbstractMap.SimpleEntry<String, String>("missing", "value"));

        boolean contains = values.entrySet().containsAll(required);
        boolean retained = values.entrySet().retainAll(keep);
        boolean removed = values.entrySet().removeAll(drop);
        return contains + "," + retained + "," + removed + ":" + values.containsKey("a") + values.containsKey("b") + values.containsKey("c") + ":" + values.size();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
