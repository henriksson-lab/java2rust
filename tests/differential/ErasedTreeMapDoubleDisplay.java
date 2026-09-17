import java.util.TreeMap;

public class ErasedTreeMapDoubleDisplay {
    public static String run() {
        TreeMap<String, Double> map = new TreeMap<String, Double>();
        map.put("z", 2.0);
        map.put("a", 1.5);
        Object value = map;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
