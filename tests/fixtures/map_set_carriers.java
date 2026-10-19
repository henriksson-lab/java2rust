import java.util.HashSet;
import java.util.Map;
import java.util.Set;

class MapSetCarriers {
    static <K, V> int keyCount(Map<K, V> values) {
        return values.keySet().size();
    }
    static <T> int sourceName(MapLike<T> value) { return value.keySet(); }
    static <K, V> Set<K> keys(Map<K, V> values) {
        return values.keySet();
    }
    static <K, V> Set<K> copiedKeys(Map<K, V> values) {
        return new HashSet<K>(values.keySet());
    }
    static <K, V> Set<K> forwardedKeys(Map<K, V> values) { return copiedKeys(values); }
    static <K, V> Set<K> maybeKeys(Map<K, V> values, boolean absent) {
        if (absent) return null;
        return values.keySet();
    }
    static <K, V> int copiedSize(Map<K, V> values) {
        return size(new HashSet<K>(values.keySet()));
    }
    static <T> int size(Set<T> values) { return values.size(); }
    static <K, V> Set<V> copiedValues(Map<K, V> values) {
        return new HashSet<V>(values.values());
    }
}

class MapLike<T> {
    int keySet() { return 7; }
}
