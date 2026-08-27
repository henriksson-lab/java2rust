import java.util.HashMap;
import java.util.Map;
public class MapGetNull2 {
    public static String run() {
        Map<String, String> m = new HashMap<String, String>();
        m.put("a", "1");
        String hit = m.get("a");
        String miss = m.get("b");
        String out = "";
        if (miss == null) { out = "miss-null"; } else { out = "miss=" + miss; }
        return hit + "," + out;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
