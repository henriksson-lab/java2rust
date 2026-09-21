import java.util.LinkedHashMap;
import java.util.Map;

public class NullableMapEntryValueCatch {
    public static String run() {
        Map<String, Integer> values = new LinkedHashMap<>();
        values.put("a", null);
        Map.Entry<String, Integer> entry = values.entrySet().iterator().next();
        String missingValue;
        try {
            missingValue = "value=" + entry.getValue().intValue();
        } catch (NullPointerException ex) {
            missingValue = "npe:" + entry.getKey();
        }
        Map<String, Integer> present = new LinkedHashMap<>();
        present.put("b", 7);
        Map.Entry<String, Integer> presentEntry = present.entrySet().iterator().next();
        Map<String, Integer> nullKeys = new LinkedHashMap<>();
        nullKeys.put(null, 1);
        nullKeys.put("x", 2);
        Map.Entry<String, Integer> nullKeyEntry = nullKeys.entrySet().iterator().next();
        String missingKey;
        try {
            missingKey = "key=" + nullKeyEntry.getKey().length();
        } catch (NullPointerException ex) {
            missingKey = "npe-key";
        }
        String before = missingValue + ":" + presentEntry.getValue().intValue() + ":" + missingKey
                + ":" + nullKeys.get("x") + ":" + nullKeys.get(null)
                + ":" + nullKeys.containsKey(null) + ":" + nullKeys.size()
                + ":" + nullKeys.getOrDefault("missing", 9)
                + ":" + nullKeys.getOrDefault(null, 9);
        boolean keySetHasNull = nullKeys.keySet().contains(null);
        boolean entrySetHasNull = nullKeys.entrySet().contains(
                new java.util.AbstractMap.SimpleEntry<String, Integer>(null, 1));
        Integer removed = nullKeys.remove("x");
        boolean removedNullKey = nullKeys.keySet().remove(null);
        Map<String, Integer> entryRemoved = new LinkedHashMap<>();
        entryRemoved.put(null, 3);
        boolean removedNullEntry = entryRemoved.entrySet().remove(
                new java.util.AbstractMap.SimpleEntry<String, Integer>(null, 3));
        return before + ":" + removed + ":" + nullKeys.size()
                + ":" + keySetHasNull + ":" + entrySetHasNull
                + ":" + removedNullKey + ":" + removedNullEntry + ":" + entryRemoved.size();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
