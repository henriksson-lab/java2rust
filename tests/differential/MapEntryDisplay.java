import java.util.AbstractMap;
import java.util.Map;

public class MapEntryDisplay {
    public static String run() {
        Map.Entry<String, String> entry = new AbstractMap.SimpleEntry<String, String>("a", "1");
        return "x" + entry;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
