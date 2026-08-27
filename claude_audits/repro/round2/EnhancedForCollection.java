import java.util.ArrayList;
import java.util.List;
public class EnhancedForCollection {
    public static String run() {
        List<String> names = new ArrayList<>();
        names.add("a");
        names.add("b");
        names.add("c");
        StringBuilder sb = new StringBuilder();
        for (String n : names) {
            if (n.equals("b")) continue;
            sb.append(n);
        }
        int count = 0;
        for (String n : names) { count += n.length(); }
        return sb.toString() + "|" + count + "|" + names.size();
    }
    public static void main(String[] args) { System.out.println(run()); }
}
