import java.util.LinkedHashSet;

public class ErasedLinkedHashSetLongDisplay {
    public static String run() {
        LinkedHashSet<Long> set = new LinkedHashSet<Long>();
        set.add(2L);
        set.add(1L);
        Object value = set;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
