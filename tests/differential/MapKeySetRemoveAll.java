import java.util.Arrays;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;

public class MapKeySetRemoveAll {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        values.put("b", "2");
        values.put("c", "3");
        Set<String> keys = values.keySet();
        boolean changed = keys.removeAll(Arrays.asList("a", "c"));
        boolean unchanged = keys.removeAll(Arrays.asList("z"));
        return changed + "," + unchanged + "," + values.size() + "," + keys.size()
            + "," + values.containsKey("a") + "," + values.containsKey("b");
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
