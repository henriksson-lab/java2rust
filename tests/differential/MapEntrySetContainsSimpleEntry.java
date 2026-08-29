import java.util.AbstractMap;
import java.util.HashMap;
import java.util.Map;

public class MapEntrySetContainsSimpleEntry {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        values.put("b", "2");
        boolean hit = values.entrySet().contains(new AbstractMap.SimpleEntry<String, String>("a", "1"));
        boolean miss = values.entrySet().contains(new AbstractMap.SimpleEntry<String, String>("a", "z"));
        return hit + "," + miss + "," + values.size();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
