import java.util.function.IntSupplier;

/** Retained inner callbacks read the live nullable outer field, not a copy. */
public class OuterReferenceProbe {
    SharedReferenceValue value;
    final IntSupplier callback;
    OuterReferenceProbe() {
        value = SharedReferenceValue.DEFAULT;
        callback = new Read();
    }
    private class Read implements IntSupplier {
        public int getAsInt() { return value.read(); }
    }
    int read() { return callback.getAsInt(); }
    public static int run() {
        OuterReferenceProbe owner = new OuterReferenceProbe();
        if (owner.read() != 7 || owner.read() != 7) return 1;
        owner.value = null;
        try { owner.read(); return 2; } catch (NullPointerException expected) { }
        owner.value = SharedReferenceValue.DEFAULT;
        if (owner.read() != 7) return 3;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}

class SharedReferenceValue {
    static final SharedReferenceValue DEFAULT = new SharedReferenceValue();
    int read() { return 7; }
}
