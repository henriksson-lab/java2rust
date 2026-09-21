import java.util.AbstractMap;

public class ErasedMapEntryTypedDisplay {
    static String show(Object value) {
        return "" + value;
    }

    public static String run() {
        Object first = new AbstractMap.SimpleEntry<Integer, String>(7, "x");
        Object second = new AbstractMap.SimpleImmutableEntry<Character, Double>('q', 1.5);
        Object third = new AbstractMap.SimpleEntry<String, String>(null, null);
        return show(first) + ";" + show(second) + ";" + show(third);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
