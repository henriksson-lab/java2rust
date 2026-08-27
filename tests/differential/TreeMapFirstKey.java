import java.util.TreeMap;

public class TreeMapFirstKey {
    public static String run() {
        TreeMap<String, Integer> m = new TreeMap<>();
        m.put("c", 3);
        m.put("a", 1);
        m.put("b", 2);
        String first = m.firstKey();
        return first + ":" + m.get(first);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
