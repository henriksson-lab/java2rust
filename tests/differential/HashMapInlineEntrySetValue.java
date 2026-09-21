import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;

public class HashMapInlineEntrySetValue {
    static Integer bump(Map.Entry<String, Integer> entry) {
        return entry.setValue(2);
    }

    static String expressionStatement() {
        HashMap<String, Integer> map = new HashMap<>();
        map.put("b", 1);
        bump(map.entrySet().iterator().next());
        return "" + map.get("b");
    }

    static String orderedMapArgument() {
        LinkedHashMap<String, Integer> map = new LinkedHashMap<>();
        map.put("z", 1);
        Integer old = bump(map.entrySet().iterator().next());
        return old + ":" + map.get("z");
    }

    public static String run() {
        HashMap<String, Integer> map = new HashMap<>();
        map.put("a", 1);
        Integer old = bump(map.entrySet().iterator().next());
        return old + ":" + map.get("a") + ":" + expressionStatement()
            + ":" + orderedMapArgument();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
