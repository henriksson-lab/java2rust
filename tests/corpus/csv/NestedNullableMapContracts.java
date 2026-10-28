import java.util.Map;
import java.util.LinkedHashMap;

class NullableMapEnvelope {
    final Map<String, Integer> values;
    NullableMapEnvelope(Map<String, Integer> values) { this.values = values; }
}
class NullableMapOwner {
    final NullableMapEnvelope envelope;
    NullableMapOwner(Map<String, Integer> values) { envelope = new NullableMapEnvelope(values); }
    Map<String, Integer> values() { return this.envelope.values; }
}
public class NestedNullableMapContracts {
    static Map<String, Integer> forward(NullableMapOwner owner) {
        return owner == null ? null : owner.values();
    }
    static int run() {
        NullableMapOwner absent = new NullableMapOwner(null);
        if (absent.values() != null || forward(absent) != null || forward(null) != null) return 1;
        Map<String, Integer> original = new LinkedHashMap<>();
        original.put("key", 7);
        NullableMapOwner present = new NullableMapOwner(original);
        Map<String, Integer> alias = forward(present);
        alias.put("key", 9);
        if (alias != original || original.get("key") != 9) return 2;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
