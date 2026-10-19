import java.util.AbstractMap;
import java.util.Map;

class EntrySetter {
    static String run() {
        Map.Entry<String, String> entry = new AbstractMap.SimpleEntry<String, String>("a", "1");
        String old = entry.setValue("2");
        return old + "," + entry;
    }
}
