import java.util.AbstractMap;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;

public class MapEntrySetAddUnsupported {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        Set<Map.Entry<String, String>> entries = values.entrySet();
        try {
            boolean changed = entries.add(new AbstractMap.SimpleEntry<String, String>("b", "2"));
            return "ok," + changed + "," + values.containsKey("b") + "," + values.size();
        } catch (UnsupportedOperationException ex) {
            return "unsupported," + values.containsKey("b") + "," + values.size();
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
