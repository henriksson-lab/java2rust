import java.util.LinkedHashMap;

public class ErasedLinkedHashMapDoubleDisplay {
    public static String run() {
        LinkedHashMap<String, Double> map = new LinkedHashMap<String, Double>();
        map.put("z", 2.0);
        map.put("a", 1.5);
        Object value = map;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
