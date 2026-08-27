import java.util.LinkedHashMap;
import java.util.Map;

public class LinkedHashMapDisplay {
    public static String run() {
        Map<String, Integer> m = new LinkedHashMap<>();
        m.put("z", 1);
        m.put("a", 2);
        return "m=" + m;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
