import java.util.AbstractMap;
import java.util.HashMap;
import java.util.Map;

public class MapEntrySetRemoveSimpleEntry {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        values.put("b", "2");
        boolean miss = values.entrySet().remove(new AbstractMap.SimpleEntry<String, String>("a", "z"));
        boolean removed = values.entrySet().remove(new AbstractMap.SimpleEntry<String, String>("a", "1"));
        return miss + "," + removed + "," + values.size() + "," + values.containsKey("a");
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
