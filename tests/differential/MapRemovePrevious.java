import java.util.HashMap;
import java.util.Map;

public class MapRemovePrevious {
    public static String run() {
        Map<String, Integer> m = new HashMap<>();
        m.put("a", 1);
        Integer missing = m.remove("b");
        Integer removed = m.remove("a");
        return missing + "," + removed + "," + m.containsKey("a");
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
