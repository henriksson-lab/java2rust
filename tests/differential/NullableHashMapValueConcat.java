import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.TreeMap;

public class NullableHashMapValueConcat {
    public static String run() {
        HashMap<String, Integer> map = new HashMap<>();
        map.put("n", null);
        map.put("p", 3);
        LinkedHashMap<String, Integer> linked = new LinkedHashMap<>();
        linked.put("n", null);
        linked.put("p", 4);
        TreeMap<String, Integer> sorted = new TreeMap<>();
        sorted.put("n", null);
        sorted.put("p", 5);
        return "" + map.get("n") + ":" + map.get("p")
            + ":" + linked.get("n") + ":" + linked.get("p")
            + ":" + sorted.get("n") + ":" + sorted.get("p");
    }

    public static void main(String[] args) { System.out.println(run()); }
}
