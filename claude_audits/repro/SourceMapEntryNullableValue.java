public class SourceMapEntryNullableValue {
    static class Entry implements java.util.Map.Entry<String, Integer> {
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
        public String toString() { return "nullable(" + key + "=" + value + ")"; }
    }

    public static String run() {
        Entry concrete = new Entry("n", null);
        java.util.Map.Entry<String, Integer> entry = concrete;
        Integer first = entry.setValue(4);
        String before = first + ":" + entry;
        Integer second = entry.setValue(null);
        return before + ":" + second + ":" + entry + ":" + concrete.getValue();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
