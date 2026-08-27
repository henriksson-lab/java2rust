import java.util.HashMap;
import java.util.Map;

public class MapEntrySetSetValue {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        String old = values.entrySet().iterator().next().setValue("2");
        return old + "," + values.get("a");
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
