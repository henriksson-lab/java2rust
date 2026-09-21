public class SourceMapEntryValue {
    static class Entry implements java.util.Map.Entry<String, Integer> {
        private final String key;
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
        public String toString() { return "custom(" + key + "=" + value + ")"; }
    }

    static String show(java.util.Map.Entry<String, Integer> entry) {
        Integer old = entry.setValue(2);
        return old + ":" + entry + ":" + entry.getKey() + ":" + entry.getValue();
    }

    public static String run() {
        Entry concrete = new Entry("k", 1);
        java.util.Map.Entry<String, Integer> entry = concrete;
        return show(entry) + ":" + concrete.getValue();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
