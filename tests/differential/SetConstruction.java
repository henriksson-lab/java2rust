import java.util.HashSet;
import java.util.Set;

public class SetConstruction {
    public static String run() {
        Set<String> a = new HashSet<String>();
        HashSet<String> b = new HashSet<>();
        Set<String> c = new HashSet<>();
        a.add("1");
        b.add("2");
        c.add("3");
        return a.size() + "," + b.size() + "," + c.size();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
