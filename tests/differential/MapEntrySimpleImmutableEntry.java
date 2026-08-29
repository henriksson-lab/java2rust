import java.util.AbstractMap;
import java.util.HashMap;
import java.util.Map;

public class MapEntrySimpleImmutableEntry {
    public static String run() {
        Map.Entry<String, String> entry = new AbstractMap.SimpleImmutableEntry<String, String>("a", "1");
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        boolean hit = values.entrySet().contains(entry);
        boolean miss = values.entrySet().contains(new AbstractMap.SimpleImmutableEntry<String, String>("a", "2"));
        return "x" + entry + "," + hit + "," + miss;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
