import java.util.AbstractMap;

public class ErasedSimpleImmutableEntryDisplay {
    public static String run() {
        Object value = new AbstractMap.SimpleImmutableEntry<String, String>("a", "1");
        return "x" + value + ":" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
