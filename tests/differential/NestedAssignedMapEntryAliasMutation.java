import java.util.LinkedHashMap;
import java.util.Map;

public class NestedAssignedMapEntryAliasMutation {
    public static String run() {
        LinkedHashMap<String, Integer> values = new LinkedHashMap<>();
        values.put("a", 1);
        Map.Entry<String, Integer> entry = values.entrySet().iterator().next();
        if (values.size() == 1) {
            Map.Entry<String, Integer> alias;
            alias = entry;
            alias.setValue(8);
        }
        return values.get("a") + ":" + entry.getValue();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
