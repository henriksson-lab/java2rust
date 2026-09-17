import java.util.TreeSet;

public class ErasedTreeSetBooleanDisplay {
    public static String run() {
        TreeSet<Boolean> set = new TreeSet<Boolean>();
        set.add(true);
        set.add(false);
        Object value = set;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
