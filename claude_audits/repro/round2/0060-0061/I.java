import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public class I {
    public String leak3(Map<String,String> m, boolean f) {
        Map<String,String> t = new HashMap<>(m);
        if (f) {
            List<String> t = new ArrayList<>();
            t.add("q");
            System.out.println(t.size());
        }
        return t.get("k");
    }

    public String noshadow(Map<String,String> m, boolean f) {
        Map<String,String> t = new HashMap<>(m);
        if (f) {
            List<String> u = new ArrayList<>();
            u.add("q");
            System.out.println(u.size());
        }
        return t.get("k");
    }
}
