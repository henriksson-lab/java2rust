import java.util.LinkedHashMap;
import java.util.Map;

public class NullableMapKeyConstructor {
    private NullableMapKeyConstructor(Map<String, Integer> values, String key) {
        values.put(key, 1);
    }

    public static String run() {
        String key = null;
        Map<String, Integer> values = new LinkedHashMap<>();
        new NullableMapKeyConstructor(values, key);
        Map<String, Integer> present = new LinkedHashMap<>();
        new NullableMapKeyConstructor(present, "x");
        return values.containsKey(null) + ":" + values.containsKey("") + ":"
                + values.get(null) + ":" + values.keySet().iterator().next() + ":"
                + present.containsKey(null) + ":" + present.get("x");
    }

    public static void main(String[] args) { System.out.println(run()); }
}
