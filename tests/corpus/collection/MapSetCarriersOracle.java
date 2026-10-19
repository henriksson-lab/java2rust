import java.util.HashMap;
import java.util.Map;
import java.util.Set;

public class MapSetCarriersOracle {
    private static final class Key {
        private final int value;
        Key(int value) { this.value = value; }
        public boolean equals(Object other) {
            return other instanceof Key && value == ((Key) other).value;
        }
        public int hashCode() { return value; }
    }
    public static void main(String[] arguments) {
        Map<Key, Key> map = new HashMap<>();
        map.put(new Key(1), new Key(7));
        map.put(new Key(2), new Key(7));
        assert MapSetCarriers.keys(map).size() == 2;
        assert MapSetCarriers.keyCount(map) == 2;
        assert MapSetCarriers.maybeKeys(map, true) == null;
        assert MapSetCarriers.maybeKeys(map, false).size() == 2;
        assert MapSetCarriers.copiedSize(map) == 2;
        assert MapSetCarriers.forwardedKeys(map).size() == 2;
        Set<Key> keys = MapSetCarriers.copiedKeys(map);
        Set<Key> values = MapSetCarriers.copiedValues(map);
        assert keys.size() == 2;
        assert values.size() == 1 && values.contains(new Key(7));
        map.put(new Key(3), new Key(8));
        assert keys.size() == 2 && values.size() == 1;
        assert MapSetCarriers.keyCount(map) == 3;
        assert MapSetCarriers.sourceName(new MapLike<Object>()) == 7;
        System.out.println("map-set-carriers: ok");
    }
}
