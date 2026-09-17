import java.util.LinkedHashSet;

public class ErasedLinkedHashSetDoubleDisplay {
    public static String run() {
        LinkedHashSet<Double> set = new LinkedHashSet<Double>();
        set.add(2.0);
        set.add(1.5);
        Object value = set;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
