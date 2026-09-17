import java.util.LinkedHashMap;

public class ErasedLinkedHashMapBooleanDisplay {
    public static String run() {
        LinkedHashMap<String, Boolean> map = new LinkedHashMap<String, Boolean>();
        map.put("z", true);
        map.put("a", false);
        Object value = map;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
