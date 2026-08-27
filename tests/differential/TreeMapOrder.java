import java.util.Map;
import java.util.TreeMap;

public class TreeMapOrder {
    public static String run() {
        Map<String, String> m = new TreeMap<>();
        m.put("c", "3");
        m.put("a", "1");
        m.put("b", "2");
        String out = "";
        for (String k : m.keySet()) {
            out = out + k + "=" + m.get(k) + ";";
        }
        return out;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
