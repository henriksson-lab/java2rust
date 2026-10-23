import java.util.function.IntSupplier;

/** A private strategy captures a final reference, not a snapshot of its state. */
public class FinalFieldCaptureProbe {
    public static CaptureOwner create() { return new CaptureOwner(new CaptureData(7)); }
    public static IntSupplier callback(CaptureOwner owner) { return owner; }
    public static int run() {
        CaptureOwner owner = create();
        IntSupplier callback = callback(owner);
        if (callback.getAsInt() != 8 || owner.current() != 8) return 1;
        if (owner.getAsInt() != 9 || owner.current() != 9) return 2;
        if (callback.getAsInt() != 10 || owner.current() != 10) return 3;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}

class CaptureData {
    int value;
    CaptureData(int value) { this.value = value; }
    int bump() { return ++value; }
}

final class CaptureOwner implements IntSupplier {
    private final CaptureData payload;
    private final IntSupplier operation;
    CaptureOwner(CaptureData payload) {
        this.payload = payload;
        operation = new Step();
    }
    private final class Step implements IntSupplier {
        public int getAsInt() { return payload.bump(); }
    }
    public int getAsInt() { return operation.getAsInt(); }
    int current() { return payload.value; }
}
