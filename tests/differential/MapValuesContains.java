import java.util.Collection;
import java.util.HashMap;
import java.util.Map;

public class MapValuesContains {
    public static String run() {
        Map<String, String> values = new HashMap<String, String>();
        values.put("a", "1");
        Collection<String> vals = values.values();
        boolean before = vals.contains("2");
        values.put("b", "2");
        boolean after = vals.contains("2");
        boolean direct = values.values().contains("1");
        return before + "," + after + "," + direct;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
