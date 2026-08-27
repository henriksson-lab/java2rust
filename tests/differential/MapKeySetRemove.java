import java.util.HashMap;
import java.util.Map;

public class MapKeySetRemove {
    public static String run() {
        Map<String, String> values = new HashMap<>();
        values.put("a", null);
        values.put("b", "2");
        boolean removed = values.keySet().remove("a");
        boolean missing = values.keySet().remove("z");
        return removed + "," + missing + "," + values.containsKey("a") + "," + values.size();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
