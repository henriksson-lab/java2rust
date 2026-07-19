import java.util.Map;

class MapEntrySetFirstKey {
    Map<String, String> values() {
        return null;
    }

    String firstKey() {
        return values().entrySet().iterator().next().getKey();
    }
}
