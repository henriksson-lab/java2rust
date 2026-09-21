public class SourceMapEntryDirect {
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
        public String toString() { return "direct(" + key + "=" + value + ")"; }
    }

    static String show(java.util.Map.Entry<String, Integer> entry) {
        return entry + ":" + entry.getValue();
    }

    public static String run() {
        return show(new Entry("k", 5));
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
