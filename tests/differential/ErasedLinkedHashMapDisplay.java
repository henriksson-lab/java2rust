import java.util.LinkedHashMap;

public class ErasedLinkedHashMapDisplay {
    public static String run() {
        LinkedHashMap<String, Integer> m = new LinkedHashMap<>();
        m.put("z", 1);
        m.put("a", 2);
        Object value = m;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
