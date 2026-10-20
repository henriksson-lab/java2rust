import java.util.*;

class RetainedMapValue {
    int value;
    RetainedMapValue(int value) { this.value = value; }
    void set(int value) { this.value = value; }
}

class RetainedMapViews {
    static Set<String> keys(Map<String, RetainedMapValue> map) { return map.keySet(); }
    static <K, V> Set<K> genericKeys(Map<K, V> map) { return map.keySet(); }
    static <K, V> Set<K> forwardKeys(Map<K, V> map) { return genericKeys(map); }
    static Object erasedGet(Map<String, Object> map, String key) { return map.get(key); }
    static Set<String> readonlyKeys(Map<String, RetainedMapValue> map) {
        return Collections.unmodifiableSet(keys(map));
    }
    static RetainedMapValue fetch(Map<String, RetainedMapValue> map, String key) {
        return map.get(key);
    }
    static void put(Map<String, RetainedMapValue> map, String key, RetainedMapValue value) {
        map.put(key, value);
    }
    static boolean remove(Set<String> view, String key) { return view.remove(key); }
    static void clear(Set<String> view) { view.clear(); }
    static boolean add(Set<String> view, String key) { return view.add(key); }
    static int run() {
        Map<String, RetainedMapValue> map = new LinkedHashMap<>();
        Map<String, RetainedMapValue> alias = map;
        RetainedMapValue first = new RetainedMapValue(1);
        RetainedMapValue second = new RetainedMapValue(2);
        put(map, "first", first);
        Set<String> live = forwardKeys(map);
        Set<String> readonly = readonlyKeys(alias);
        put(alias, "second", second);
        if (live.size() != 2 || readonly.size() != 2 || !readonly.contains("second")) return 1;
        RetainedMapValue retained = fetch(map, "first");
        if (retained != first) return 2;
        if (!remove(live, "first") || alias.containsKey("first")) return 3;
        retained.set(7);
        if (first.value != 7 || readonly.contains("first")) return 4;
        try { add(live, "forbidden"); return 5; }
        catch (UnsupportedOperationException expected) { }
        try { remove(readonly, "missing"); return 6; }
        catch (UnsupportedOperationException expected) { }
        Iterator<String> stable = live.iterator();
        put(map, "second", first);
        if (!stable.next().equals("second") || fetch(alias, "second") != first) return 7;
        Iterator<String> invalidated = readonly.iterator();
        put(map, "third", second);
        try { invalidated.next(); return 8; }
        catch (ConcurrentModificationException expected) { }
        Iterator<String> cursor = live.iterator();
        String removed = cursor.next();
        cursor.remove();
        if (map.containsKey(removed) || readonly.size() != 1) return 9;
        Iterator<String> readonlyCursor = readonly.iterator();
        try { readonlyCursor.remove(); return 10; }
        catch (UnsupportedOperationException expected) { }
        clear(live);
        if (!alias.isEmpty() || !readonly.isEmpty()) return 11;
        try { clear(readonly); return 12; }
        catch (UnsupportedOperationException expected) { }
        put(alias, "again", retained);
        if (!readonly.contains("again") || fetch(map, "again").value != 7) return 13;
        Map<String, RetainedMapValue> copied = new LinkedHashMap<>(map);
        Set<String> copiedKeys = keys(copied);
        clear(live);
        if (!copiedKeys.contains("again") || !readonly.isEmpty()) return 14;
        fetch(copied, "again").set(8);
        if (first.value != 8) return 15;
        Map<String, RetainedMapValue> nullable = new LinkedHashMap<>();
        put(nullable, null, first);
        put(nullable, "null-value", null);
        Set<String> nullableKeys = keys(nullable);
        if (!nullableKeys.contains(null) || fetch(nullable, null) != first) return 16;
        if (fetch(nullable, "missing") != null || fetch(nullable, "null-value") != null) return 17;
        if (!nullable.containsKey("null-value") || nullable.containsKey("missing")) return 18;
        if (!remove(nullableKeys, null) || nullable.containsKey(null)) return 19;
        Map<String, Object> erased = new LinkedHashMap<>();
        Map<String, Object> erasedAlias = erased;
        erased.put("node", first);
        Object held = erasedGet(erasedAlias, "node");
        erased.remove("node");
        if (held != first || !erasedAlias.isEmpty()) return 20;
        return 0;
    }
}
