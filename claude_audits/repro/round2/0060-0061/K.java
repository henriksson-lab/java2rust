import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public class K {
    public String f(Map<String,String> m) {
        try {
            List<String> v = new ArrayList<>();
            v.add("a");
            return v.get(0);
        } catch (RuntimeException e) {
            String v = m.get("k");
            return v;
        }
    }

    public String g(Map<String,String> m) {
        try {
            return "ok";
        } catch (RuntimeException e) {
            String v = m.get("k");
            return v;
        }
    }
}
