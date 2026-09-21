import java.util.HashMap;
import java.util.Map;

public class MixedSourceAndHashMapEntryAlias {
    public static String run() {
        HashMap<String, Integer> map = new HashMap<>();
        map.put("a", 1);
        Map.Entry<String, Integer> entry = map.entrySet().iterator().next();
        Map.Entry<String, Integer> alias = entry;
        alias.setValue(2);
        return entry.getValue() + ":" + map.get("a");
    }

    public static void main(String[] args) { System.out.println(run()); }
}
