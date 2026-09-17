import java.util.AbstractMap;

public class ErasedMapEntryBooleanDisplay {
    public static String run() {
        Object value = new AbstractMap.SimpleEntry<String, Boolean>("a", true);
        return "x" + value + ":" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
