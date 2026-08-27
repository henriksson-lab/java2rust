import java.util.HashMap;
import java.util.Map;

public class MapGetOrDefault {
    public static String run() {
        Map<String, Integer> m = new HashMap<String, Integer>();
        m.put("a", 1);
        return m.getOrDefault("a", 9) + "," + m.getOrDefault("z", 9);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
