import java.util.Map;
import java.util.LinkedHashMap;
import java.util.TreeMap;

public class HeaderMapContracts {
    static Map<String, Integer> held;
    static Map<String, Integer> create(boolean insensitive) {
        return insensitive ? new TreeMap<>(String.CASE_INSENSITIVE_ORDER) : new LinkedHashMap<>();
    }
    static void put(Map<String, Integer> map, String key, int value) { map.put(key, value); }
    static int run() {
        Map<String, Integer> map = create(true);
        held = map;
        String original = new String("Header");
        put(map, original, 1);
        if (map.put("HEADER", 2) != 1 || held.size() != 1 || held.get("header") != 2) return 1;
        if (map.keySet().iterator().next() != original) return 2;
        put(map, "\u0130", 3);
        if (map.get("\u0131") != 3 || map.get("i") != 3) return 3;
        put(map, "\ud801\udc00", 4);
        if (map.get("\ud801\udc28") != 4) return 4;
        put(map, "\ud800", 5);
        put(map, "\udc00", 6);
        if (map.get("\ud800") != 5 || map.get("\udc00") != 6) return 5;
        put(map, "\u00df", 7);
        put(map, "SS", 8);
        if (map.get("\u00df") != 7 || map.get("ss") != 8) return 6;
        Map<String, Integer> sensitive = create(false);
        put(sensitive, "A", 1); put(sensitive, "a", 2);
        if (sensitive.size() != 2) return 7;
        if (map.remove("HEADER") != 2 || held.containsKey("header")) return 8;
        String[] expected = new String[]{"\u0130", "SS", "\u00df", "\ud800", "\udc00", "\ud801\udc00"};
        int index = 0;
        for (String key : map.keySet()) {
            if (!key.equals(expected[index++])) return 9;
        }
        if (index != expected.length) return 10;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
