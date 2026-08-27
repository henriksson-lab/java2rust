import java.util.HashMap;
import java.util.Map;
import java.util.Set;

public class MapKeySetClear {
    public static String run() {
        Map<String, String> values = new HashMap<>();
        values.put("a", "1");
        values.put("b", "2");
        Set<String> keys = values.keySet();
        keys.clear();
        return values.containsKey("a") + "," + values.size() + "," + keys.size();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
