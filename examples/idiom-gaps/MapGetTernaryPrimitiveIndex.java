import java.util.Map;

class MapGetTernaryPrimitiveIndex {
    int indexOrMissing(Map<String, Integer> indexes, String name) {
        int idx = indexes.containsKey(name) ? indexes.get(name) : -1;
        return idx;
    }
}
