import java.util.HashMap;
import java.util.Map;

public class MapPutPrevious {
    public static String run() {
        Map<String, Integer> m = new HashMap<>();
        Integer p1 = m.put("a", 1);
        Integer p2 = m.put("a", 2);
        return p1 + "," + p2;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
