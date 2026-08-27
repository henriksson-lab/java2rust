import java.util.HashMap;
import java.util.Map;
import java.util.Set;

public class MapKeySetAliasRemove {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        values.put("b", "2");
        Set<String> keys = values.keySet();
        boolean removed = keys.remove("a");
        return removed + "," + values.containsKey("a") + "," + values.size();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
