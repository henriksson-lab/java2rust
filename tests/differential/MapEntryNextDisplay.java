import java.util.LinkedHashMap;
import java.util.Map;

public class MapEntryNextDisplay {
    public static String run() {
        Map<String, String> values = new LinkedHashMap<String, String>();
        values.put("a", "1");
        return "x" + values.entrySet().iterator().next();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
