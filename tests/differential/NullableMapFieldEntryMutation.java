import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.TreeMap;

public class NullableMapFieldEntryMutation {
    private HashMap<String, Integer> hash = new HashMap<>();
    private LinkedHashMap<String, Integer> linked = new LinkedHashMap<>();
    private TreeMap<String, Integer> tree = new TreeMap<>();

    String execute() {
        hash.put(null, null);
        Map.Entry<String, Integer> hashEntry = hash.entrySet().iterator().next();
        Integer hashOld = hashEntry.setValue(7);

        linked.put(null, null);
        Map.Entry<String, Integer> linkedEntry = linked.entrySet().iterator().next();
        Integer linkedOld = linkedEntry.setValue(8);

        tree.put("a", null);
        Map.Entry<String, Integer> treeEntry = tree.entrySet().iterator().next();
        Integer treeOld = treeEntry.setValue(9);

        return hashOld + ":" + hash.get(null)
            + "," + linkedOld + ":" + linked.get(null)
            + "," + treeOld + ":" + tree.get("a") + ":" + tree.firstKey();
    }

    public static String run() {
        NullableMapFieldEntryMutation probe = new NullableMapFieldEntryMutation();
        return probe.execute();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
