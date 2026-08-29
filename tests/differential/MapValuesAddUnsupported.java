import java.util.ArrayList;
import java.util.Collection;
import java.util.HashMap;
import java.util.Map;

public class MapValuesAddUnsupported {
    public static String run() {
        Map<String, String> map = new HashMap<>();
        map.put("a", "1");
        Collection<String> values = map.values();
        StringBuilder out = new StringBuilder();
        try {
            values.add("x");
            out.append("add-ok");
        } catch (UnsupportedOperationException ex) {
            out.append("add-unsupported");
        }
        out.append(",").append(map.size()).append(",").append(map.containsValue("x"));

        ArrayList<String> extra = new ArrayList<>();
        extra.add("y");
        try {
            values.addAll(extra);
            out.append(",addAll-ok");
        } catch (UnsupportedOperationException ex) {
            out.append(",addAll-unsupported");
        }
        out.append(",").append(map.size()).append(",").append(map.containsValue("y"));
        return out.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
