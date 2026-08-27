import java.util.HashMap;
import java.util.Map;

public class MapPutIfAbsent {
    public static String run() {
        Map<String, Integer> m = new HashMap<String, Integer>();
        Integer first = m.putIfAbsent("a", 1);
        Integer second = m.putIfAbsent("a", 2);
        return first + "," + second + "," + m.get("a");
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
