import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.TreeMap;

public class NullableStaticMapFieldEntryMutation {
    private static HashMap<String, Integer> hash = new HashMap<>();
    private static LinkedHashMap<String, Integer> linked = new LinkedHashMap<>();
    private static TreeMap<String, Integer> tree = new TreeMap<>();

    public static String run() {
        hash.clear();
        linked.clear();
        tree.clear();
        hash.put(null, null);
        Map.Entry<String, Integer> hashEntry = hash.entrySet().iterator().next();
        hashEntry.setValue(7);
        linked.put(null, null);
        Map.Entry<String, Integer> linkedEntry = linked.entrySet().iterator().next();
        linkedEntry.setValue(8);
        tree.put("a", null);
        Map.Entry<String, Integer> treeEntry = tree.entrySet().iterator().next();
        treeEntry.setValue(9);
        return hash.get(null) + ":" + hashEntry.getValue()
            + "," + linked.get(null) + ":" + linkedEntry.getValue()
            + "," + tree.get("a") + ":" + treeEntry.getValue();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
