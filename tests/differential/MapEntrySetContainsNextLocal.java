import java.util.HashMap;
import java.util.Map;

public class MapEntrySetContainsNextLocal {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        Map.Entry<String, String> entry = values.entrySet().iterator().next();
        boolean hit = values.entrySet().contains(entry);
        return hit + "," + values.size();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
