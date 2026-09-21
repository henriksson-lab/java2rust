import java.util.LinkedHashMap;
import java.util.Map;

public class MixedSourceAndNativeMapEntry {
    static class Entry implements Map.Entry<String, Integer> {
        private final String key;
        private Integer value;

        Entry(String key, Integer value) { this.key = key; this.value = value; }
        public String getKey() { return key; }
        public Integer getValue() { return value; }
        public Integer setValue(Integer value) {
            Integer old = this.value;
            this.value = value;
            return old;
        }
        public String toString() { return "source(" + key + "=" + value + ")"; }
    }

    static Integer bump(Map.Entry<String, Integer> entry) {
        return entry.setValue(10);
    }

    public static String run() {
        LinkedHashMap<String, Integer> map = new LinkedHashMap<>();
        map.put("a", 1);
        Map.Entry<String, Integer> nativeEntry = map.entrySet().iterator().next();
        Entry concrete = new Entry("k", 2);
        Map.Entry<String, Integer> sourceEntry = concrete;
        Integer nativeOld = nativeEntry.setValue(3);
        Integer sourceOld = sourceEntry.setValue(4);
        map.put("a", 5);
        Integer observed = nativeEntry.getValue();
        String live = nativeEntry.toString();
        Map.Entry<String, Integer> nativeAlias = nativeEntry;
        Integer aliasOld = nativeAlias.setValue(8);
        Integer aliasObserved = nativeEntry.getValue();
        map.put("a", 5);
        Integer passedOld = bump(nativeEntry);
        Integer passedSeen = map.get("a");
        map.put("a", 5);
        boolean contains = map.entrySet().contains(nativeEntry);
        boolean removed = map.entrySet().remove(nativeEntry);
        map.put("a", 7);
        Integer detached = nativeEntry.getValue();
        Integer detachedOld = nativeEntry.setValue(9);
        Integer replacement = map.get("a");
        LinkedHashMap<String, Integer> nullable = new LinkedHashMap<>();
        nullable.put(null, null);
        Map.Entry<String, Integer> nullableEntry = nullable.entrySet().iterator().next();
        String nullableBefore = nullableEntry.toString();
        boolean nullKey = nullableEntry.getKey() == null;
        boolean nullValueBefore = nullableEntry.getValue() == null;
        Integer nullableOld = nullableEntry.setValue(6);
        nullable.put(null, null);
        Integer nullablePassedOld = bump(nullableEntry);
        nullable.put(null, null);
        String nullableAfter = nullableEntry.toString();
        boolean nullValueAfter = nullableEntry.getValue() == null;
        boolean nullableContains = nullable.entrySet().contains(nullableEntry);
        boolean emptyCaught;
        try {
            LinkedHashMap<String, Integer> empty = new LinkedHashMap<>();
            Map.Entry<String, Integer> absent = empty.entrySet().iterator().next();
            emptyCaught = false;
        } catch (java.util.NoSuchElementException ex) {
            emptyCaught = true;
        }
        return nativeOld + ":" + nativeEntry.getKey() + ":" + live + ":" + observed
                + ":" + contains + ":" + removed + ":" + map.size()
                + ":" + sourceOld + ":" + sourceEntry + ":" + concrete.getValue()
                + ":" + detached + ":" + detachedOld + ":" + nativeEntry.getValue()
                + ":" + replacement + ":" + nullableBefore + ":" + nullableOld
                + ":" + nullableEntry.getKey() + ":" + nullableAfter + ":" + nullableContains
                + ":" + nullKey + ":" + nullValueBefore + ":" + nullValueAfter
                + ":" + emptyCaught + ":" + aliasOld + ":" + aliasObserved
                + ":" + passedOld + ":" + passedSeen + ":" + nullablePassedOld;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
