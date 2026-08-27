import java.util.Collection;
import java.util.HashMap;
import java.util.Map;

public class MapValuesAliasRemove {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        values.put("b", "2");
        Collection<String> vals = values.values();
        boolean removed = vals.remove("1");
        return removed + "," + values.containsKey("a") + "," + values.size();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
