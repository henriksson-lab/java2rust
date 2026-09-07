import java.util.AbstractMap;

public class ErasedMapEntryDisplay {
    public static String run() {
        Object value = new AbstractMap.SimpleEntry<String, String>("a", "1");
        return "x" + value + ":" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
