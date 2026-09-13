import java.util.ArrayList;
import java.util.List;

public class EnhancedForConcurrentModification {
    public static String run() {
        List<String> xs = new ArrayList<String>();
        xs.add("a");
        try {
            for (String s : xs) {
                xs.add(s);
            }
            return "no";
        } catch (java.util.ConcurrentModificationException e) {
            return "cme";
        }
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
