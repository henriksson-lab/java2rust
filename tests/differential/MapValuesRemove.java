import java.util.HashMap;
import java.util.Map;

public class MapValuesRemove {
    public static String run() {
        Map<String, String> values = new HashMap<>();
        values.put("a", "x");
        values.put("b", "y");
        boolean removed = values.values().remove("x");
        boolean missing = values.values().remove("z");
        return removed + "," + missing + "," + values.containsKey("a") + "," + values.size();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
