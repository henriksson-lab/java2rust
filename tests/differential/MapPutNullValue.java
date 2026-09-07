import java.util.HashMap;
import java.util.Map;

public class MapPutNullValue {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", null);
        values.put("b", "x");
        return (values.get("a") == null) + ":" + values.get("a") + ":" + values.get("b");
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
