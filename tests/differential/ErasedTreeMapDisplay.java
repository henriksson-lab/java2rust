import java.util.TreeMap;

public class ErasedTreeMapDisplay {
    public static String run() {
        TreeMap<String, Integer> m = new TreeMap<>();
        m.put("z", 1);
        m.put("a", 2);
        Object value = m;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
