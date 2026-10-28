import java.util.Map;
import java.util.LinkedHashMap;

public class GenericMapInsertContracts {
    private String[] values;
    private static Map<String, String> shared = new LinkedHashMap<>();
    private Map<String, Integer> headers = new LinkedHashMap<>();
    GenericMapInsertContracts(String[] values) { this.values = values; headers.put("first", 0); headers.put(null, 1); }
    <M extends Map<String, String>> M putIn(M target) {
        target.put("first", values[0]);
        target.put("present-null", values[1]);
        return target;
    }
    static <M extends Map<String, String>> M forward(GenericMapInsertContracts owner, M target) {
        return owner.putIn(target);
    }
    static Map<String, String> fresh(GenericMapInsertContracts owner) { return owner.putIn(new LinkedHashMap<>()); }
    <M extends Map<String, String>> M putViaHeaders(M target) {
        headers.forEach((key, value) -> target.put(key, values[value]));
        return target;
    }
    static <M extends Map<String, String>> M unrelated(M target) { target.put("safe", "safe"); return target; }
    public static int run() {
        String[] values = new String[]{"value", null};
        GenericMapInsertContracts owner = new GenericMapInsertContracts(values);
        Map<String, String> target = new LinkedHashMap<>();
        Map<String, String> alias = target;
        Map<String, String> result = forward(owner, target);
        if (result != target || alias != result) return 1;
        if (!"value".equals(alias.get("first"))) return 2;
        if (!result.containsKey("present-null") || result.get("present-null") != null) return 3;
        values[0] = "changed";
        if (forward(owner, alias) != target || !"changed".equals(target.get("first"))) return 4;
        Map<String, String> fresh = fresh(owner);
        if (fresh == target || !fresh.containsKey("present-null") || fresh.get("present-null") != null) return 5;
        if (forward(owner, shared) != shared || !"changed".equals(shared.get("first"))) return 6;
        if (!shared.containsKey("present-null") || shared.get("present-null") != null) return 7;
        if (owner.putViaHeaders(target) != alias || !alias.containsKey(null) || alias.get(null) != null) return 8;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
