import java.util.*;

class MapViewValue {
    int value;
    MapViewValue(int value) { this.value = value; }
}

class RetainedMapValues {
    static <K, V> Collection<V> values(Map<K, V> map) { return map.values(); }
    static <K, V> Collection<V> forwardValues(Map<K, V> map) { return values(map); }
    static <K, V> V putAbsent(Map<K, V> map, K key, V value) {
        return map.putIfAbsent(key, value);
    }
    static int run() {
        Map<String, MapViewValue> map = new LinkedHashMap<>();
        Map<String, MapViewValue> alias = map;
        MapViewValue first = new MapViewValue(1);
        MapViewValue second = new MapViewValue(2);
        MapViewValue third = new MapViewValue(3);
        map.put("a", first);
        map.put("b", first);
        map.put("nil", null);
        Collection<MapViewValue> live = forwardValues(map);
        Collection<MapViewValue> readonly = Collections.unmodifiableCollection(live);
        Iterator<MapViewValue> stable = live.iterator();
        if (putAbsent(alias, "a", second) != first) return 1;
        if (stable.next() != first || putAbsent(map, "nil", second) != null) return 2;
        if (stable.next() != first || stable.next() != second) return 3;
        Iterator<MapViewValue> invalidated = live.iterator();
        if (putAbsent(alias, "new", third) != null || live.size() != 4) return 4;
        try { invalidated.next(); return 5; }
        catch (ConcurrentModificationException expected) { }
        if (!live.remove(first) || map.containsKey("a") || !alias.containsKey("b")) return 6;
        Iterator<MapViewValue> cursor = live.iterator();
        MapViewValue held = cursor.next();
        cursor.remove();
        held.value = 7;
        if (held != first || first.value != 7 || map.containsKey("b")) return 7;
        int sum = 0;
        for (MapViewValue value : map.values()) sum += value.value;
        if (sum != 5 || readonly.size() != 2) return 8;
        Collection<MapViewValue> direct = alias.values();
        map.put("null-again", null);
        if (!direct.remove(null) || alias.containsKey("null-again")) return 9;
        try { live.add(first); return 10; }
        catch (UnsupportedOperationException expected) { }
        try { readonly.remove(first); return 11; }
        catch (UnsupportedOperationException expected) { }
        live.clear();
        if (!map.isEmpty() || !readonly.isEmpty()) return 12;
        try { readonly.clear(); return 13; }
        catch (UnsupportedOperationException expected) { }
        return 0;
    }
}
