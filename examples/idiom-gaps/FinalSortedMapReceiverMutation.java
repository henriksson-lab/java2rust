import java.util.SortedMap;
import java.util.TreeMap;

class FinalSortedMapReceiverMutation {
    SortedMap<String, Integer> build(String key, int value) {
        final SortedMap<String, Integer> out = new TreeMap<>();
        out.put(key, value);
        return out;
    }
}
