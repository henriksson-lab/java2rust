import java.util.Map;
import java.util.TreeMap;

public class TreeMapEntryParameterMutation {
    static Integer bump(TreeMap<String, Integer> map) {
        Map.Entry<String, Integer> entry = map.entrySet().iterator().next();
        return entry.setValue(7);
    }

    public static String run() {
        TreeMap<String, Integer> map = new TreeMap<>();
        map.put("z", 2);
        map.put("a", 1);
        Integer previous = bump(map);
        return previous + ":" + map.get("a") + ":" + map.firstKey();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
