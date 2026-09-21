import java.util.LinkedHashMap;
import java.util.Map;

public class NullableMapKeyParameter {
    private static String insert(Map<String, Integer> values, String key) {
        values.put(key, 1);
        return values.containsKey(null) + ":" + values.containsKey("") + ":"
                + values.get(null) + ":" + values.keySet().iterator().next();
    }

    private static String insertLiteral(Map<String, Integer> values) {
        values.put(null, 3);
        return values.get(null) + ":" + values.keySet().iterator().next();
    }

    public static String run() {
        String key = null;
        Map<String, Integer> values = new LinkedHashMap<>();
        Map<String, Integer> present = new LinkedHashMap<>();
        Map<String, Integer> literal = new LinkedHashMap<>();
        return insert(values, key) + ":" + insert(present, "x") + ":"
                + insertLiteral(literal);
    }

    public static void main(String[] args) { System.out.println(run()); }
}
