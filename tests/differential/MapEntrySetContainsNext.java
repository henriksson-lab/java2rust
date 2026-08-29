import java.util.HashMap;
import java.util.Map;

public class MapEntrySetContainsNext {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        boolean hit = values.entrySet().contains(values.entrySet().iterator().next());
        return hit + "," + values.size();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
