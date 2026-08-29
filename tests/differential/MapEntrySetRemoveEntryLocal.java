import java.util.AbstractMap;
import java.util.HashMap;
import java.util.Map;

public class MapEntrySetRemoveEntryLocal {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        Map.Entry<String, String> entry = new AbstractMap.SimpleEntry<String, String>("a", "1");
        boolean removed = values.entrySet().remove(entry);
        return removed + "," + values.size() + "," + values.containsKey("a");
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
