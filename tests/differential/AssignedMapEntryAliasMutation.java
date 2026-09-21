import java.util.LinkedHashMap;
import java.util.Map;

public class AssignedMapEntryAliasMutation {
    public static String run() {
        LinkedHashMap<String, Integer> values = new LinkedHashMap<>();
        values.put("a", 1);
        Map.Entry<String, Integer> entry = values.entrySet().iterator().next();
        Map.Entry<String, Integer> alias;
        alias = entry;
        for (int i = 0; i < 1; i++) {
            alias.setValue(6);
        }
        return values.get("a") + ":" + entry.getValue() + ":" + alias.getValue();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
