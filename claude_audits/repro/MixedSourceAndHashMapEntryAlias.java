import java.util.HashMap;
import java.util.Map;

public class MixedSourceAndHashMapEntryAlias {
    static class Entry implements Map.Entry<String, Integer> {
        String key;
        Integer value;
        Entry(String key, Integer value) { this.key = key; this.value = value; }
        public String getKey() { return key; }
        public Integer getValue() { return value; }
        public Integer setValue(Integer value) {
            Integer old = this.value;
            this.value = value;
            return old;
        }
    }

    public static String run() {
        HashMap<String, Integer> map = new HashMap<>();
        map.put("a", 1);
        Map.Entry<String, Integer> entry = map.entrySet().iterator().next();
        Map.Entry<String, Integer> alias = entry;
        alias.setValue(2);
        return entry.getValue() + ":" + map.get("a");
    }

    public static void main(String[] args) { System.out.println(run()); }
}
