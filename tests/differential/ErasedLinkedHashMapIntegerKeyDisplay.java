import java.util.LinkedHashMap;

public class ErasedLinkedHashMapIntegerKeyDisplay {
    public static String run() {
        LinkedHashMap<Integer, String> map = new LinkedHashMap<Integer, String>();
        map.put(2, "z");
        map.put(1, "a");
        Object value = map;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
