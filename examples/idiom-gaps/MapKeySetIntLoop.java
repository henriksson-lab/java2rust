import java.util.Map;

class MapKeySetIntLoop {
    String pick(Map<String, String> values, int start, int limit) {
        java.util.List<String> keys = values.keySet();
        int i = start;
        while (i < values.size() && i < start + limit) {
            return keys.get(i);
        }
        return "";
    }
}
