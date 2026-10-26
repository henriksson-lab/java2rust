import java.util.HashMap;
class NullableLookup<K> {
    HashMap<K, Integer> values = new HashMap<K, Integer>();
    void put(K key, int value) { values.put(key, value); }
    public int read(K key) { return values.get(key); }
    public boolean contains(K key) { return values.containsKey(key); }
    public int remove(K key) { return values.remove(key); }
}
class GenericNullableMapLookupProbe {
    static int run() {
        NullableLookup<String> lookup = new NullableLookup<String>();
        String token = "key";
        lookup.put(null, 3);
        lookup.put(token, 4);
        if (lookup.read(null) != 3 || !lookup.contains(null) || lookup.read(token) != 4) return 1;
        if (lookup.remove(null) != 3 || lookup.contains(null)) return 2;
        if (lookup.remove(token) != 4 || lookup.contains(token)) return 3;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
