import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.TreeMap;

public class MapFieldEntryMutation {
    private HashMap<String, Integer> hash = new HashMap<>();
    private LinkedHashMap<String, Integer> linked = new LinkedHashMap<>();
    private TreeMap<String, Integer> tree = new TreeMap<>();

    String execute() {
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

    static String run() {
        MapFieldEntryMutation probe = new MapFieldEntryMutation();
        return probe.execute();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
