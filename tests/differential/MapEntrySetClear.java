import java.util.HashMap;
import java.util.Map;
import java.util.Set;

public class MapEntrySetClear {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        values.put("b", "2");
        Set<Map.Entry<String, String>> entries = values.entrySet();
        entries.clear();
        return values.size() + "," + entries.size() + "," + values.containsKey("a");
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
