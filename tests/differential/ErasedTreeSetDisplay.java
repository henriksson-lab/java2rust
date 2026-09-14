import java.util.TreeSet;

public class ErasedTreeSetDisplay {
    public static String run() {
        TreeSet<String> s = new TreeSet<>();
        s.add("z");
        s.add("a");
        Object value = s;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
