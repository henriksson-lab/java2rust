import java.util.*;

class ProtocolKey<T> {
    T value;
    ProtocolKey(T value) { this.value = value; }
    public boolean equals(Object other) {
        T snapshot = value;
        return snapshot != null && snapshot.hashCode() != 12345678 && other != null;
    }
    public int hashCode() { return 0; }
}

class ProtocolChild<U> extends ProtocolKey<U> {
    ProtocolChild(U value) { super(value); }
}

class ProtocolIndex<V> {
    Map<ProtocolKey<V>, Integer> entries;
    ProtocolIndex(Map<ProtocolKey<V>, Integer> entries) { this.entries = entries; }
    Integer read(ProtocolKey<V> key) { return entries.get(key); }
    Integer erase(ProtocolKey<V> key) { return entries.remove(key); }
    boolean contains(ProtocolKey<V> key) { return entries.containsKey(key); }
    int sizeOnly() { return entries.size(); }
}

class NestedKeyCapabilities {
    static <T> Integer forward(ProtocolIndex<T> index, ProtocolKey<T> key) {
        return index.read(key);
    }
    static int run() {
        Map<ProtocolKey<String>, Integer> entries = new LinkedHashMap<>();
        Set<ProtocolKey<String>> keys = entries.keySet();
        ProtocolIndex<String> index = new ProtocolIndex<>(entries);
        ProtocolKey<String> stored = new ProtocolChild<>("stored");
        ProtocolKey<String> query = new ProtocolKey<>("query");
        entries.put(stored, 7);
        if (!index.contains(query) || forward(index, query) != 7) return 1;
        if (index.erase(query) != 7 || index.sizeOnly() != 0 || !keys.isEmpty()) return 2;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
