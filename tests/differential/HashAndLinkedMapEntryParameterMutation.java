import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;

public class HashAndLinkedMapEntryParameterMutation {
    static Integer bumpHash(HashMap<String, Integer> map) {
        Map.Entry<String, Integer> entry = map.entrySet().iterator().next();
        return entry.setValue(7);
    }

    static Integer bumpLinked(LinkedHashMap<String, Integer> map) {
        Map.Entry<String, Integer> entry = map.entrySet().iterator().next();
        return entry.setValue(9);
    }

    public static String run() {
        HashMap<String, Integer> hash = new HashMap<>();
        hash.put("x", 1);
        Integer oldHash = bumpHash(hash);
        LinkedHashMap<String, Integer> linked = new LinkedHashMap<>();
        linked.put("b", 2);
        linked.put("a", 3);
        Integer oldLinked = bumpLinked(linked);
        return oldHash + ":" + hash.get("x") + ":" + oldLinked + ":"
            + linked.get("b") + ":" + linked.keySet().iterator().next();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
