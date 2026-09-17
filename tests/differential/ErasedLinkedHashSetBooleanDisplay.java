import java.util.LinkedHashSet;

public class ErasedLinkedHashSetBooleanDisplay {
    public static String run() {
        LinkedHashSet<Boolean> set = new LinkedHashSet<Boolean>();
        set.add(true);
        set.add(false);
        Object value = set;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
