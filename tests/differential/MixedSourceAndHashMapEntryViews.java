import java.util.HashMap;
import java.util.Map;

public class MixedSourceAndHashMapEntryViews {
    public static String run() {
        HashMap<String, Integer> map = new HashMap<>();
        map.put("a", 1);
        Map.Entry<String, Integer> entry = map.entrySet().iterator().next();
        boolean before = map.entrySet().contains(entry);
        map.put("a", 3);
        int seen = entry.getValue();
        boolean removed = map.entrySet().remove(entry);
        return before + ":" + seen + ":" + removed + ":" + map.containsKey("a");
    }

    public static void main(String[] args) { System.out.println(run()); }
}
