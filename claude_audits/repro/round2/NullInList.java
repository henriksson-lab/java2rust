import java.util.ArrayList;
import java.util.List;
public class NullInList {
    public static String run() {
        List<String> xs = new ArrayList<>();
        xs.add("a");
        xs.add(null);
        StringBuilder sb = new StringBuilder();
        for (String s : xs) { sb.append(s == null ? "N" : s); }
        return sb.toString() + "," + xs.size() + "," + (xs.get(1) == null);
    }
    public static void main(String[] args) { System.out.println(run()); }
}
