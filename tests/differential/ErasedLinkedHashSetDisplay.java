import java.util.LinkedHashSet;

public class ErasedLinkedHashSetDisplay {
    public static String run() {
        LinkedHashSet<String> s = new LinkedHashSet<>();
        s.add("z");
        s.add("a");
        Object value = s;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
