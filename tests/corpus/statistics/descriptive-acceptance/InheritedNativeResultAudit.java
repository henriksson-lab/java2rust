import java.util.function.DoubleSupplier;
import java.util.function.IntSupplier;

public class InheritedNativeResultAudit {
    private static int calls;
    private static boolean fail;

    interface Value extends DoubleSupplier, IntSupplier {
        default int getAsInt() { return (int) getAsDouble(); }
    }

    interface Derived extends Value {}

    static class Constant implements Derived {
        private final double value;
        Constant(double value) { this.value = value; }
        public double getAsDouble() { return value; }
    }

    static Value forwarded(DoubleSupplier value) { return value::getAsDouble; }

    public static int run() {
        calls = 0;
        fail = false;
        Derived value = new Constant(4.5);
        if (value.getAsDouble() != 4.5 || value.getAsInt() != 4) return 1;
        Value copy = forwarded(() -> {
            calls++;
            if (fail) throw new IllegalArgumentException("callback");
            return 8.5;
        });
        if (copy.getAsDouble() != 8.5 || copy.getAsInt() != 8 || calls != 2) return 2;
        fail = true;
        try {
            copy.getAsInt();
            return 3;
        } catch (IllegalArgumentException expected) {
            if (!"callback".equals(expected.getMessage()) || calls != 3) return 4;
        }
        fail = false;
        if (copy.getAsInt() != 8 || calls != 4) return 5;
        return 0;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
