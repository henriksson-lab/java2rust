import java.util.AbstractMap;

public class ErasedMapEntryIntegerDisplay {
    public static String run() {
        Object value = new AbstractMap.SimpleEntry<String, Integer>("a", 1);
        return "x" + value + ":" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
