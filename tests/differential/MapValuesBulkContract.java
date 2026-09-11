import java.util.Arrays;
import java.util.HashMap;
import java.util.Map;

public class MapValuesBulkContract {
    public static String run() {
        Map<String, String> values = new HashMap<>();
        values.put("a", "1");
        values.put("b", "2");
        values.put("c", "3");

        boolean contains = values.values().containsAll(Arrays.asList("1", "3"));
        boolean changed = values.values().retainAll(Arrays.asList("2", "3"));
        return contains + "," + changed + "," + values.containsKey("a") + values.containsKey("b") + values.containsKey("c") + ":" + values.size();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
