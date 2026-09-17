import java.util.AbstractMap;

public class ErasedMapEntryDoubleDisplay {
    public static String run() {
        Object value = new AbstractMap.SimpleEntry<String, Double>("a", 1.5);
        return "x" + value + ":" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
