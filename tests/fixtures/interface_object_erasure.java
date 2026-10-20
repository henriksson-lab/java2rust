interface ObjectCarrierPort<T> {
    T get();
    void set(T value);
}
interface ObjectCarrierChild extends ObjectCarrierPort<String> { int id(); }
interface ObjectCarrierRead<T> { T get(); }
interface ObjectCarrierTag<T> { int code(); }
interface ObjectCarrierTask { int code(); }

class MutableObjectCarrier implements ObjectCarrierChild {
    String value = "old";
    public String get() { return value; }
    public void set(String next) { value = next; }
    public int id() { return 3; }
}
class ImmutableObjectCarrier implements ObjectCarrierRead<String> {
    String value;
    ImmutableObjectCarrier(String value) { this.value = value; }
    public String get() { return value; }
}

class InterfaceObjectErasure {
    static ObjectCarrierTask task() { return () -> 9; }
    static ObjectCarrierTask anonymousTask() {
        return new ObjectCarrierTask() { public int code() { return 11; } };
    }
    static Object erase(ObjectCarrierPort<String> value) { return value; }
    static Object eraseChild(ObjectCarrierChild value) { return value; }
    static Object eraseRead(ObjectCarrierRead<String> value) { return value; }
    static int run() {
        MutableObjectCarrier original = new MutableObjectCarrier();
        ObjectCarrierChild child = original;
        ObjectCarrierPort<String> parent = child;
        MutableObjectCarrier recovered = (MutableObjectCarrier) erase(parent);
        recovered.set("new");
        if (!original.get().equals("new")) return 1;
        if (recovered != original) return 2;
        if ((MutableObjectCarrier) eraseChild(child) != original) return 3;
        ImmutableObjectCarrier immutable = new ImmutableObjectCarrier("same");
        ObjectCarrierRead<String> reader = immutable;
        if ((ImmutableObjectCarrier) eraseRead(reader) != immutable) return 4;
        ObjectCarrierPort<String> missing = null;
        if (erase(missing) != null) return 5;
        if (task().code() != 9 || anonymousTask().code() != 11) return 6;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
