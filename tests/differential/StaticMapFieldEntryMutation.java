import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.TreeMap;

public class StaticMapFieldEntryMutation {
    private static HashMap<String, Integer> hash = new HashMap<>();
    private static LinkedHashMap<String, Integer> linked = new LinkedHashMap<>();
    private static TreeMap<String, Integer> tree = new TreeMap<>();

    public static String run() {
        hash.clear();
        linked.clear();
        tree.clear();
        hash.put("h", 1);
        Map.Entry<String, Integer> hashEntry = hash.entrySet().iterator().next();
        Integer hashOld = hashEntry.setValue(7);
        linked.put("l", 2);
        Map.Entry<String, Integer> linkedEntry = linked.entrySet().iterator().next();
        Integer linkedOld = linkedEntry.setValue(8);
        tree.put("z", 3);
        tree.put("a", 4);
        Map.Entry<String, Integer> treeEntry = tree.entrySet().iterator().next();
        Integer treeOld = treeEntry.setValue(9);
        return hashOld + ":" + hash.get("h") + ":" + hashEntry.getValue()
            + "," + linkedOld + ":" + linked.get("l") + ":" + linkedEntry.getValue()
            + "," + treeOld + ":" + tree.get("a") + ":" + treeEntry.getValue()
            + ":" + tree.firstKey();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
