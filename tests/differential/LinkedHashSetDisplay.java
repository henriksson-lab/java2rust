import java.util.LinkedHashSet;

public class LinkedHashSetDisplay {
    public static String run() {
        LinkedHashSet<String> s = new LinkedHashSet<>();
        s.add("z");
        s.add("a");
        return "s=" + s;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
