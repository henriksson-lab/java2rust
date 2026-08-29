import java.util.HashMap;
import java.util.Map;

public class MapEntrySetEnhancedForSetValue {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        String old = "";
        for (Map.Entry<String, String> entry : values.entrySet()) {
            old = entry.setValue("2");
        }
        return old + "," + values.get("a");
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
