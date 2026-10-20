import java.util.*;

class NativeComparisonEffects {
    int calls;
    Set<String> touch(Set<String> value) { calls++; return value; }
}

class NativeCollectionIdentity {
    static <T> boolean sameSet(Set<T> left, Set<T> right) { return left == right; }
    static <K,V> boolean sameMap(Map<K,V> left, Map<K,V> right) { return left == right; }
    static <T> boolean equalSet(Set<T> left, Set<T> right) { return left.equals(right); }
    static <T> boolean equalSetSafely(Set<T> left, Set<T> right) { return Objects.equals(left, right); }
    static int run() {
        Set<String> original = new HashSet<>();
        Set<String> alias = original;
        Set<String> other = new HashSet<>();
        original.add("value");
        other.add("value");
        if (original == other || alias != original || !original.equals(other)) return 1;
        if (!equalSet(original, other)) return 11;
        if (!equalSetSafely(original, other) || !equalSetSafely(null, null)) return 13;
        Set<String> nullableElements = new HashSet<>();
        nullableElements.add(null);
        if (original == nullableElements) return 12;
        Set<String> nullable = null;
        nullable = original;
        if (nullable != alias) return 2;
        nullable = null;
        if (nullable == alias || !sameSet(null, null) || sameSet(original, null)) return 3;
        List<Set<String>> groups = new ArrayList<>();
        groups.add(original);
        for (Set<String> group : groups) if (group != original) return 4;
        Map<String,Set<String>> index = new HashMap<>();
        index.put("value", original);
        if (index.get("value") != original) return 5;
        Map<String,Integer> firstMap = new HashMap<>();
        Map<String,Integer> mapAlias = firstMap;
        Map<String,Integer> secondMap = new HashMap<>();
        if (sameMap(firstMap, secondMap) || !sameMap(firstMap, mapAlias)) return 6;
        List<String> firstList = new ArrayList<>();
        List<String> listAlias = firstList;
        List<String> secondList = new ArrayList<>();
        firstList.add("value");
        secondList.add("value");
        if (firstList == secondList || listAlias != firstList || !firstList.equals(secondList)) return 7;
        if (new HashSet<String>() == new HashSet<String>()) return 8;
        NativeComparisonEffects effects = new NativeComparisonEffects();
        Set<String> missing = null;
        try { missing.equals(effects.touch(other)); return 9; }
        catch (NullPointerException expected) { }
        if (effects.calls != 1 || !Objects.equals(missing, missing)
            || Objects.equals(missing, original) || !Objects.equals(original, other)) return 10;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
