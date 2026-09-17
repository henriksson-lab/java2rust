import java.util.TreeMap;

public class ErasedTreeMapIntegerKeyDisplay {
    public static String run() {
        TreeMap<Integer, String> map = new TreeMap<Integer, String>();
        map.put(2, "z");
        map.put(1, "a");
        Object value = map;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
