import java.util.Map;

class MapEntryArrays {
    static Object[] erased(Map<String, String> values) {
        return values.entrySet().toArray();
    }
}
