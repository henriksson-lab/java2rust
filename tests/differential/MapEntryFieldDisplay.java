import java.util.AbstractMap;
import java.util.Map;

public class MapEntryFieldDisplay {
    Map.Entry<String, String> entry = new AbstractMap.SimpleEntry<String, String>("a", "1");

    public String show() {
        return entry.getKey() + "=" + entry.getValue();
    }

    public static String run() {
        return new MapEntryFieldDisplay().show();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
