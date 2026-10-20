import java.util.*;

interface DispatchCapability<T> {
    boolean same(T left, T right);
    int count();
}

interface DispatchSize<T> {
    int count();
}

class DispatchCapabilityBase<A> {
    int calls;
    public boolean same(A left, A right) { return false; }
    public boolean forwarded(A left, A right) { calls += 0; return same(left, right); }
}

class DispatchCapabilityImpl<T> extends DispatchCapabilityBase<T> implements DispatchCapability<T>, DispatchSize<T> {
    public boolean same(T left, T right) { calls++; return left == right; }
    public int count() { return calls; }
}

class DispatchCapabilityValue {
    int value;
    DispatchCapabilityValue(int value) { this.value = value; }
}

class InheritedDispatchCapabilities {
    static <T> DispatchCapability<T> asCapability(DispatchCapabilityImpl<T> value) {
        return value;
    }
    static <T> DispatchSize<T> asSize(DispatchCapabilityImpl<T> value) { return value; }
    static <T> boolean forward(DispatchCapabilityImpl<T> value, T left, T right) {
        return value.forwarded(left, right);
    }
    static <T> List<DispatchCapability<T>> stored(DispatchCapabilityImpl<T> value) {
        List<DispatchCapability<T>> values = new ArrayList<>();
        values.add(value);
        return values;
    }
    static <T> Map<String, DispatchCapability<T>> mapped(DispatchCapabilityImpl<T> value) {
        Map<String, DispatchCapability<T>> values = new HashMap<>();
        values.put("value", value);
        return values;
    }
    static int run() {
        DispatchCapabilityValue first = new DispatchCapabilityValue(7);
        DispatchCapabilityValue second = new DispatchCapabilityValue(7);
        DispatchCapabilityImpl<DispatchCapabilityValue> implementation = new DispatchCapabilityImpl<>();
        DispatchCapabilityImpl<DispatchCapabilityValue> alias = implementation;
        if (!implementation.forwarded(first, first) || alias.forwarded(first, second)) return 1;
        DispatchCapability<DispatchCapabilityValue> view = asCapability(implementation);
        if (!view.same(first, first) || view.same(first, second)) return 2;
        if (!forward(alias, first, first) || asSize(alias).count() != 5) return 3;
        List<DispatchCapability<DispatchCapabilityValue>> list = stored(alias);
        Map<String, DispatchCapability<DispatchCapabilityValue>> map = mapped(alias);
        if (list.get(0).count() != 5) return 4;
        alias.same(first, first);
        if (map.get("value").count() != 6 || list.get(0).count() != 6) return 5;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
