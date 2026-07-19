import java.util.Map;

class CollectionFieldMutator {
    Map<String, String> values;

    void putValue(String key, String value) {
        values.put(key, value);
    }
}
