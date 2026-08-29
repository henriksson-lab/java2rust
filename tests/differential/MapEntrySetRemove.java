import java.util.HashMap;
import java.util.Map;

public class MapEntrySetRemove {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        boolean removed = values.entrySet().remove(values.entrySet().iterator().next());
        return removed + "," + values.size() + "," + values.containsKey("a");
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
