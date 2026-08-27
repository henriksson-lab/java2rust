import java.util.HashSet;
import java.util.Set;

public class SetRemove {
    public static String run() {
        Set<String> s = new HashSet<String>();
        s.add("a");
        s.add("b");
        boolean missing = s.remove("z");
        boolean removed = s.remove("a");
        return missing + "," + removed + "," + s.size();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
