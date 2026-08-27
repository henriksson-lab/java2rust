import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public class M {
    public String fin(Map<String,String> m) {
        try {
            List<String> v = new ArrayList<>();
            v.add("a");
            return v.get(0);
        } finally {
            String v = m.get("k");
            System.out.println(v);
        }
    }
}
