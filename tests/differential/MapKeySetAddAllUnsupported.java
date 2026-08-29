import java.util.Arrays;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;

public class MapKeySetAddAllUnsupported {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        Set<String> keys = values.keySet();
        try {
            boolean changed = keys.addAll(Arrays.asList("b"));
            return "ok," + changed + "," + values.containsKey("b") + "," + values.size();
        } catch (UnsupportedOperationException ex) {
            return "unsupported," + values.containsKey("b") + "," + values.size();
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
