import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.TreeMap;

public class NullableMapEntryParameterMutation {
    static Integer bumpHash(HashMap<String, Integer> map) {
        Map.Entry<String, Integer> entry = map.entrySet().iterator().next();
        return entry.setValue(7);
    }

    static Integer bumpLinked(LinkedHashMap<String, Integer> map) {
        Map.Entry<String, Integer> entry = map.entrySet().iterator().next();
        return entry.setValue(8);
    }

    static Integer bumpTree(TreeMap<String, Integer> map) {
        Map.Entry<String, Integer> entry = map.entrySet().iterator().next();
        return entry.setValue(9);
    }

    public static String run() {
        HashMap<String, Integer> hash = new HashMap<>();
        hash.put(null, null);
        Integer hashOld = bumpHash(hash);

        LinkedHashMap<String, Integer> linked = new LinkedHashMap<>();
        linked.put(null, null);
        Integer linkedOld = bumpLinked(linked);

        TreeMap<String, Integer> tree = new TreeMap<>();
        tree.put("t", null);
        Integer treeOld = bumpTree(tree);

        return hashOld + ":" + hash.get(null)
            + "," + linkedOld + ":" + linked.get(null)
            + "," + treeOld + ":" + tree.get("t") + ":" + tree.firstKey();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
