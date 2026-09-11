import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;

public class MapKeySetBulkContract {
    public static String run() {
        Map<String, String> values = new HashMap<>();
        values.put("a", "1");
        values.put("b", "2");
        values.put("c", "3");

        Set<String> required = new HashSet<>();
        required.add("a");
        required.add("c");

        Set<String> keep = new HashSet<>();
        keep.add("b");
        keep.add("c");

        boolean contains = values.keySet().containsAll(required);
        boolean changed = values.keySet().retainAll(keep);
        return contains + "," + changed + "," + values.containsKey("a") + values.containsKey("b") + values.containsKey("c") + ":" + values.size();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
