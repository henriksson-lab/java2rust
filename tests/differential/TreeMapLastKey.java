import java.util.TreeMap;

public class TreeMapLastKey {
    public static String run() {
        TreeMap<String, Integer> m = new TreeMap<>();
        m.put("c", 3);
        m.put("a", 1);
        m.put("b", 2);
        String last = m.lastKey();
        return last + ":" + m.get(last);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
