import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public class F {
    static void takeString(String s) {}

    public String one(List<String> xs, Map<String,String> m, boolean b) {
        String a = m.get("k");
        StringBuilder sb = new StringBuilder();
        sb.append(a);
        List<String> c = new ArrayList<>();
        c.add(a);
        return sb.toString();
    }

    public String three(List<String> xs, Map<String,String> m, boolean b) {
        String a = m.get("k");
        StringBuilder sb = new StringBuilder();
        sb.append(a);
        List<String> c = new ArrayList<>();
        c.add(a);
        if (b) {
            String a = "z";
            StringBuilder sb = new StringBuilder();
            List<String> c = new ArrayList<>();
            sb.append(a);
            c.add(a);
        }
        return sb.toString();
    }
}
