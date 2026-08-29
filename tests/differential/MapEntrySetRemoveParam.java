import java.util.AbstractMap;
import java.util.HashMap;
import java.util.Map;

public class MapEntrySetRemoveParam {
    static boolean remove(Map<String, String> values, Map.Entry<String, String> entry) {
        return values.entrySet().remove(entry);
    }

    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        boolean miss = remove(values, new AbstractMap.SimpleEntry<String, String>("a", "2"));
        boolean hit = remove(values, new AbstractMap.SimpleEntry<String, String>("a", "1"));
        return miss + "," + hit + "," + values.size() + "," + values.containsKey("a");
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
