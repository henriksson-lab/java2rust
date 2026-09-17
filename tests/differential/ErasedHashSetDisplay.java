import java.util.HashSet;

public class ErasedHashSetDisplay {
    public static String run() {
        HashSet<String> set = new HashSet<String>();
        set.add("a");
        Object value = set;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
