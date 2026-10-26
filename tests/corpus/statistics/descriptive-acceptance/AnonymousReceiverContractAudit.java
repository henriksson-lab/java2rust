import java.util.function.DoubleSupplier;
import java.util.function.LongSupplier;

public class AnonymousReceiverContractAudit {
    interface Reading extends DoubleSupplier, LongSupplier {}

    static class Counter implements Reading {
        int count;
        public double getAsDouble() { return ++count; }
        public long getAsLong() { return ++count; }
    }

    static Reading fixed(int value) {
        return new Reading() {
            public double getAsDouble() { return value; }
            public long getAsLong() { return value + 1L; }
        };
    }

    public static int run() {
        Reading counter = new Counter();
        if (counter.getAsDouble() != 1 || counter.getAsLong() != 2) return 1;
        Reading value = fixed(7);
        Reading alias = value;
        if (value.getAsDouble() != 7 || alias.getAsLong() != 8) return 2;
        if (counter.getAsDouble() != 3 || value.getAsDouble() != 7) return 3;
        return 0;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
