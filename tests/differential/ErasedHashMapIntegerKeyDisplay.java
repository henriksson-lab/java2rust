import java.util.HashMap;

public class ErasedHashMapIntegerKeyDisplay {
    public static String run() {
        HashMap<Integer, String> map = new HashMap<Integer, String>();
        map.put(1, "a");
        Object value = map;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
