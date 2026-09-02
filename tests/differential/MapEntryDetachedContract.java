import java.util.HashMap;
import java.util.Map;

public class MapEntryDetachedContract {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        Map.Entry<String, String> entry = values.entrySet().iterator().next();
        values.remove("a");
        values.put("a", "2");
        boolean hit = values.entrySet().contains(entry);
        boolean removed = values.entrySet().remove(entry);
        return hit + "," + removed + "," + values.get("a");
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
