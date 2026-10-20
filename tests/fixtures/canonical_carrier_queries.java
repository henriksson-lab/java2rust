import java.util.*;

class QueryOwner {
    int calls;
    <T> boolean matches(T value) { calls++; return value == this; }
    <T> boolean reverse(T value) { calls++; return this == value; }
}

class CanonicalCarrierQueries {
    static <K> int present(Map<K, String> map, Set<K> probes) {
        int count = 0;
        for (K probe : probes) {
            if (map.containsKey(probe)) count++;
        }
        return count;
    }
    static int run() {
        Map<String, String> map = new LinkedHashMap<>();
        Map<String, String> alias = map;
        map.put("a", "value");
        Set<String> probes = new LinkedHashSet<>();
        probes.add("a");
        probes.add("missing");
        probes.add(null);
        if (present(alias, probes) != 1) return 1;
        if (alias.get(null) != null || alias.remove(null) != null) return 2;
        if (map.size() != 1 || !alias.containsKey("a")) return 3;
        Set<String> live = map.keySet();
        if (!live.remove("a") || map.containsKey("a")) return 8;
        map.put("null-value", null);
        if (!live.remove("null-value") || alias.containsKey("null-value")) return 9;
        QueryOwner owner = new QueryOwner();
        QueryOwner other = new QueryOwner();
        if (!owner.matches(owner) || owner.matches(other)) return 4;
        if (!owner.reverse(owner) || owner.reverse(other)) return 5;
        if (owner.matches(null) || owner.reverse(null)) return 6;
        if (owner.calls != 6 || other.calls != 0) return 7;
        return 0;
    }
}
