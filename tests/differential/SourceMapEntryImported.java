import java.util.Map;

public class SourceMapEntryImported {
    static class Entry implements Map.Entry<String, Integer> {
        private String key;
        private Integer value;

        Entry(String key, Integer value) {
            this.key = key;
            this.value = value;
        }

        public String getKey() { return key; }
        public Integer getValue() { return value; }
        public Integer setValue(Integer value) {
            Integer old = this.value;
            this.value = value;
            return old;
        }
        public String toString() { return "imported(" + key + "=" + value + ")"; }
    }

    public static String run() {
        Entry concrete = new Entry("k", 1);
        Map.Entry<String, Integer> entry = concrete;
        Integer old = entry.setValue(3);
        return old + ":" + entry + ":" + concrete.getValue();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
