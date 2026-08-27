import java.util.LinkedHashMap;
import java.util.Map;

public class LinkedHashMapOrder {
    public static String run() {
        Map<String, String> m = new LinkedHashMap<>();
        m.put("z", "1");
        m.put("a", "2");
        m.put("m", "3");
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
