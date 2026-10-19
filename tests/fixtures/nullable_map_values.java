import java.util.HashMap;
import java.util.Map;

interface TextProvider {
    String text(boolean present);
}

interface StoredValue { int number(); }
class NumericStoredValue implements StoredValue {
    public int number() { return 7; }
}

class NullableTextProvider implements TextProvider {
    public String text(boolean present) { return present ? "data" : null; }
    StoredValue item(boolean present) { return present ? new NumericStoredValue() : null; }
}

class MapReceiver {
    private final Map<String, String> values;
    MapReceiver(Map<String, String> values) { this.values = values; }
    int inspect() { return NullableMapValues.inspect(values); }
}

class NullableMapValues {
    private final Map<String, String> stored = new HashMap<>();

    NullableMapValues(NullableTextProvider provider) {
        stored.put("missing", provider.text(false));
        stored.put("present", provider.text(true));
    }

    static Map<String, String> build(NullableTextProvider provider) {
        Map<String, String> values = new HashMap<>();
        values.put("missing", provider.text(false));
        values.put("present", provider.text(true));
        return values;
    }

    static Map<String, String> buildViaInterface(TextProvider provider) {
        Map<String, String> values = new HashMap<>();
        values.put("missing", provider.text(false));
        values.put("present", provider.text(true));
        return values;
    }

    static int inspect(Map<String, String> values) {
        if (!values.containsKey("missing")) return -1;
        if (values.get("missing") != null) return -2;
        return values.size() * 100 + values.get("present").length();
    }

    static int concrete() { return inspect(build(new NullableTextProvider())); }
    static int viaInterface() { return inspect(buildViaInterface(new NullableTextProvider())); }
    static int constructorArgument() {
        return new MapReceiver(build(new NullableTextProvider())).inspect();
    }
    static int field() {
        NullableMapValues owner = new NullableMapValues(new NullableTextProvider());
        return inspect(owner.stored);
    }
    static int interfaceValues() {
        NullableTextProvider provider = new NullableTextProvider();
        Map<String, StoredValue> values = new HashMap<>();
        values.put("missing", provider.item(false));
        values.put("present", provider.item(true));
        if (!values.containsKey("missing") || values.get("missing") != null) return -1;
        StoredValue value = values.get("present");
        return value.number() + values.get("present").number();
    }
}
