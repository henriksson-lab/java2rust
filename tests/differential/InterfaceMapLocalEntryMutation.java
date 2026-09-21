import java.util.HashMap;
import java.util.Map;
import java.util.SortedMap;
import java.util.TreeMap;

public class InterfaceMapLocalEntryMutation {
    public static String run() {
        Map<String, Integer> values = new HashMap<>();
        values.put(null, null);
        Map.Entry<String, Integer> valueEntry = values.entrySet().iterator().next();
        Integer valueOld = valueEntry.setValue(7);

        SortedMap<String, Integer> sorted = new TreeMap<>();
        sorted.put("a", null);
        Map.Entry<String, Integer> sortedEntry = sorted.entrySet().iterator().next();
        Integer sortedOld = sortedEntry.setValue(8);

        return valueOld + ":" + values.size() + ":" + values.get(null)
            + ":" + valueEntry.getValue() + "," + sortedOld + ":"
            + sorted.size() + ":" + sorted.get("a") + ":" + sortedEntry.getValue();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
