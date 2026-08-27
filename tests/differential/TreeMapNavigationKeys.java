import java.util.TreeMap;

public class TreeMapNavigationKeys {
    public static String run() {
        TreeMap<String, Integer> m = new TreeMap<>();
        m.put("b", 2);
        m.put("d", 4);

        return m.lowerKey("b")
            + "," + m.lowerKey("c")
            + "," + m.floorKey("c")
            + "," + m.floorKey("d")
            + "," + m.ceilingKey("a")
            + "," + m.ceilingKey("c")
            + "," + m.higherKey("c")
            + "," + m.higherKey("d");
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
