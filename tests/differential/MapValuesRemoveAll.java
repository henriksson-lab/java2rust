import java.util.Arrays;
import java.util.Collection;
import java.util.HashMap;
import java.util.Map;

public class MapValuesRemoveAll {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        values.put("b", "2");
        values.put("c", "3");
        Collection<String> vals = values.values();
        boolean changed = vals.removeAll(Arrays.asList("1", "3"));
        boolean unchanged = vals.removeAll(Arrays.asList("9"));
        return changed + "," + unchanged + "," + values.size() + "," + vals.size()
            + "," + values.containsKey("a") + "," + values.containsKey("b");
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
