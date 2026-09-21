import java.util.LinkedHashMap;
import java.util.Map;

public class NullableMapKeyVariable {
    public static String run() {
        String key = null;
        Map<String, Integer> values = new LinkedHashMap<>();
        values.put(key, 1);
        return values.containsKey(null) + ":" + values.containsKey("") + ":"
                + values.get(null) + ":" + values.keySet().iterator().next();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
