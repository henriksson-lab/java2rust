import java.util.LinkedHashMap;
import java.util.Map;
import java.util.NoSuchElementException;

public class MapEntryIteratorNextCatch {
    public static String run() {
        Map<String, String> values = new LinkedHashMap<String, String>();
        try {
            return values.entrySet().iterator().next().getKey();
        } catch (NoSuchElementException e) {
            values.put("a", "1");
        }
        return values.entrySet().iterator().next().getKey();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
