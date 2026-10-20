import java.util.*;

class StoredNode {
    int value;
    StoredNode(int value) { this.value = value; }
    void update(int value) { this.value = value; }
    int read() { return value; }
}

class StoredLeaf extends StoredNode {
    StoredLeaf(int value) { super(value); }
    void update(int value) { this.value = value * 2; }
}

class SourceCarrierMapValues {
    static StoredNode fetch(Map<String, StoredNode> map, String key) { return map.get(key); }
    static StoredNode remove(Map<String, StoredNode> map, String key) { return map.remove(key); }
    static StoredNode replace(Map<String, StoredNode> map, String key, StoredNode value) {
        return map.put(key, value);
    }
    static int run() {
        Map<String, StoredNode> map = new LinkedHashMap<>();
        Map<String, StoredNode> alias = map;
        Set<String> keys = map.keySet();
        StoredNode node = new StoredLeaf(3);
        map.put("a", node);
        StoredNode read = fetch(alias, "a");
        if (read != node) return 1;
        read.update(9);
        if (node.read() != 18 || fetch(map, "a").read() != 18) return 2;
        StoredNode next = new StoredNode(5);
        if (replace(alias, "a", next) != node || fetch(map, "a") != next) return 3;
        if (remove(alias, "a") != next || !keys.isEmpty()) return 4;
        map.put("null", null);
        if (fetch(alias, "null") != null || fetch(alias, "absent") != null) return 5;
        StoredNode missing = fetch(alias, "absent");
        int calls = 0;
        try { missing.update(calls++); return 6; }
        catch (NullPointerException expected) { }
        if (calls != 1) return 7;
        List<StoredNode> list = new ArrayList<>();
        list.add(next);
        list.get(0).update(6);
        if (next.read() != 6) return 8;
        List<StoredNode> nullable = new ArrayList<>();
        nullable.add(null);
        calls = 0;
        try { nullable.get(0).update(calls++); return 9; }
        catch (NullPointerException expected) { }
        if (calls != 1) return 10;
        calls = 0;
        try { list.get(9).update(calls++); return 11; }
        catch (IndexOutOfBoundsException expected) { }
        if (calls != 0) return 12;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
