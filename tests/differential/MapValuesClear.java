import java.util.Collection;
import java.util.HashMap;
import java.util.Map;

public class MapValuesClear {
    public static String run() {
        Map<String, String> values = new HashMap<>();
        values.put("a", "1");
        values.put("b", "2");
        Collection<String> vals = values.values();
        vals.clear();
        return values.size() + "," + vals.size() + "," + values.containsKey("a");
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
