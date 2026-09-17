import java.util.LinkedHashSet;

public class ErasedLinkedHashSetFloatDisplay {
    public static String run() {
        LinkedHashSet<Float> set = new LinkedHashSet<Float>();
        set.add(2.0f);
        set.add(1.5f);
        Object value = set;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
