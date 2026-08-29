import java.util.AbstractMap;
import java.util.HashMap;
import java.util.Map;

public class MapEntrySetContainsParam {
    static boolean has(Map<String, String> values, Map.Entry<String, String> entry) {
        return values.entrySet().contains(entry);
    }

    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        boolean hit = has(values, new AbstractMap.SimpleEntry<String, String>("a", "1"));
        boolean miss = has(values, new AbstractMap.SimpleEntry<String, String>("a", "2"));
        return hit + "," + miss + "," + values.size();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
