public class SourceMapEntryErased {
    static class Entry implements java.util.Map.Entry<String, Integer> {
        private final String key;
        private Integer value;

        Entry(String key, Integer value) { this.key = key; this.value = value; }
        public String getKey() { return key; }
        public Integer getValue() { return value; }
        public Integer setValue(Integer value) { Integer old = this.value; this.value = value; return old; }
        public String toString() { return "entry(" + key + "=" + value + ")"; }
    }

    static String show(Object object) { return object + ":" + object.toString(); }

    public static String run() {
        java.util.Map.Entry<String, Integer> entry = new Entry("k", 5);
        Object object = entry;
        Object alias = object;
        return show(alias);
    }

    public static void main(String[] args) { System.out.println(run()); }
}
