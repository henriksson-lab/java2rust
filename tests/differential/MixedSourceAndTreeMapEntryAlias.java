import java.util.Map;
import java.util.TreeMap;

public class MixedSourceAndTreeMapEntryAlias {
    static Integer bump(Map.Entry<String, Integer> entry) {
        return entry.setValue(3);
    }

    static String nullableValue() {
        TreeMap<String, Integer> map = new TreeMap<>();
        map.put("a", null);
        Map.Entry<String, Integer> entry = map.entrySet().iterator().next();
        try {
            int value = entry.getValue();
            return "missed:" + value;
        } catch (NullPointerException expected) {
            return "caught";
        }
    }

    public static String run() {
        TreeMap<String, Integer> map = new TreeMap<>();
        map.put("b", 2);
        map.put("a", 1);
        Map.Entry<String, Integer> entry = map.entrySet().iterator().next();
        Map.Entry<String, Integer> alias = entry;
        Integer old = bump(alias);
        String updated = entry.getKey() + ":" + old + ":" + map.get("a");
        map.put("a", 5);
        String replaced = ":" + entry.getValue();
        boolean live = map.entrySet().contains(entry);
        map.remove("a");
        map.put("a", 4);
        boolean detached = map.entrySet().contains(entry);
        return updated + replaced + ":" + entry.getValue() + ":" + map.firstKey()
            + ":" + map.keySet().iterator().next() + ":" + map.lowerKey("b")
            + ":" + map.floorKey("b") + ":" + map.ceilingKey("a")
            + ":" + map.higherKey("a") + ":" + live + ":" + detached
            + ":" + nullableValue();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
