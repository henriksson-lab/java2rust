import java.util.HashMap;

public class ErasedHashMapDisplay {
    public static String run() {
        HashMap<String, Integer> map = new HashMap<String, Integer>();
        map.put("a", 1);
        Object value = map;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
