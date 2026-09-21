import java.util.HashMap;
import java.util.Map;

public class NullableHashMapEntryUnboxingCatch {
    public static String run() {
        HashMap<String, Integer> map = new HashMap<>();
        map.put("a", null);
        Map.Entry<String, Integer> entry = map.entrySet().iterator().next();
        try {
            int value = entry.getValue();
            return "wrong:" + value;
        } catch (NullPointerException ex) {
            return "caught:" + map.containsKey("a");
        }
    }

    public static void main(String[] args) { System.out.println(run()); }
}
