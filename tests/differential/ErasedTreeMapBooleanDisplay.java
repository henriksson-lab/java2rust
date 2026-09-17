import java.util.TreeMap;

public class ErasedTreeMapBooleanDisplay {
    public static String run() {
        TreeMap<String, Boolean> map = new TreeMap<String, Boolean>();
        map.put("z", true);
        map.put("a", false);
        Object value = map;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
