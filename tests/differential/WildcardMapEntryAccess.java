import java.util.AbstractMap;
import java.util.Map;

public class WildcardMapEntryAccess {
    static Object key(Map.Entry<?, ?> entry) {
        return entry.getKey();
    }

    static Object value(Map.Entry<?, ?> entry) {
        return entry.getValue();
    }

    static String show(Map.Entry<?, ?> entry) {
        return "entry=" + entry;
    }

    public static String run() {
        Map.Entry<?, ?> strings =
            new AbstractMap.SimpleEntry<String, String>("a", "1");
        Map.Entry<?, ?> mixed =
            new AbstractMap.SimpleEntry<Integer, Double>(7, 1.5);
        return show(strings) + ":" + key(strings) + ":" + value(strings)
            + ";" + show(mixed) + ":" + key(mixed) + ":" + value(mixed);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
