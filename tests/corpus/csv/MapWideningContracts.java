import java.util.LinkedHashMap;
import java.util.Map;

public class MapWideningContracts {
    static <K, V> Map<K, V> widen(LinkedHashMap<K, V> source) { return source; }
    public static int run() {
        LinkedHashMap<String, String[]> concrete = new LinkedHashMap<>();
        String[] row = new String[]{"before", null};
        concrete.put("row", row);
        concrete.put("present-null", null);
        Map<String, String[]> viewed = concrete;
        Map<String, String[]> forwarded = widen(concrete);
        if (viewed != concrete || forwarded != concrete) return 1;
        if (forwarded.get("row") != row) return 2;
        viewed.get("row")[0] = "after";
        if (!"after".equals(row[0]) || !"after".equals(concrete.get("row")[0])) return 3;
        if (!forwarded.containsKey("present-null") || forwarded.get("present-null") != null) return 4;
        viewed.remove("row");
        if (concrete.containsKey("row") || forwarded.containsKey("row")) return 5;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
