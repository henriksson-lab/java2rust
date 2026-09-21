public class GenericSourceMapEntry {
    static class Entry<K, V> implements java.util.Map.Entry<K, V> {
        private final K key;
        private V value;

        Entry(K key, V value) { this.key = key; this.value = value; }
        public K getKey() { return key; }
        public V getValue() { return value; }
        public V setValue(V value) {
            V old = this.value;
            this.value = value;
            return old;
        }
        public String toString() { return "generic(" + key + "=" + value + ")"; }
    }

    static String show(java.util.Map.Entry<String, Integer> entry) {
        Integer old = entry.setValue(2);
        return old + ":" + entry + ":" + entry.getKey() + ":" + entry.getValue();
    }

    public static String run() {
        Entry<String, Integer> concrete = new Entry<>("k", 1);
        java.util.Map.Entry<String, Integer> entry = concrete;
        String before = show(entry) + ":" + concrete.getValue();
        Integer replaced = entry.setValue(null);
        return before + ":" + replaced + ":" + concrete.getValue();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
