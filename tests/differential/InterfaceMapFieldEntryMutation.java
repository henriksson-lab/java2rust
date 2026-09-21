import java.util.HashMap;
import java.util.Map;
import java.util.SortedMap;
import java.util.TreeMap;

public class InterfaceMapFieldEntryMutation {
    private Map<String, Integer> values = new HashMap<>();
    private SortedMap<String, Integer> sorted = new TreeMap<>();
    private static Map<String, Integer> staticValues = new HashMap<>();

    String mutate() {
        values.clear();
        values.put(null, null);
        Map.Entry<String, Integer> valueEntry = values.entrySet().iterator().next();
        Integer valueOld = valueEntry.setValue(7);

        sorted.clear();
        sorted.put("a", null);
        Map.Entry<String, Integer> sortedEntry = sorted.entrySet().iterator().next();
        Integer sortedOld = sortedEntry.setValue(8);

        return valueOld + ":" + values.get(null) + ":" + valueEntry.getValue()
            + "," + sortedOld + ":" + sorted.get("a") + ":" + sortedEntry.getValue();
    }

    static String mutateStatic() {
        staticValues.clear();
        staticValues.put(null, null);
        Map.Entry<String, Integer> entry = staticValues.entrySet().iterator().next();
        Integer old = entry.setValue(9);
        return old + ":" + staticValues.get(null) + ":" + entry.getValue();
    }

    public static String run() {
        return new InterfaceMapFieldEntryMutation().mutate() + "," + mutateStatic();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
