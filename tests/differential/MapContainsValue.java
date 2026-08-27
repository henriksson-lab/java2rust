import java.util.HashMap;
import java.util.Map;

public class MapContainsValue {
    public static String run() {
        Map<String, Integer> m = new HashMap<String, Integer>();
        m.put("a", 1);
        m.put("b", 2);
        return m.containsValue(2) + "," + m.containsValue(3);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
