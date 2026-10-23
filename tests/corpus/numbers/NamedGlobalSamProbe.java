package namedglobalsam;

import java.util.function.DoubleUnaryOperator;

/** Caller/storage regression, not a numerical algorithm replacement. */
public class NamedGlobalSamProbe {
    static final DoubleUnaryOperator GLOBAL = make(3);
    static DoubleUnaryOperator make(double offset) { return new Operation(offset); }
    static DoubleUnaryOperator adapt(Operation value) { return value; }
    static class Payload {
        final double offset;
        Payload(double offset) { this.offset = offset; }
        double read() { return offset; }
    }
    static class Base implements DoubleUnaryOperator {
        final Payload payload;
        Base(double offset) { payload = new Payload(offset); }
        public double applyAsDouble(double x) { return payload.read() + x; }
    }
    static final class Operation extends Base {
        Operation(double offset) { super(offset); }
    }
    public static int run() {
        DoubleUnaryOperator first = GLOBAL;
        if (first != GLOBAL || first.applyAsDouble(4) != 7) return 1;
        if (first.applyAsDouble(-2) != 1 || first.applyAsDouble(4) != 7) return 2;
        Operation source = new Operation(10);
        DoubleUnaryOperator a = adapt(source);
        DoubleUnaryOperator b = adapt(source);
        if (a != b || a.applyAsDouble(2) != 12 || b.applyAsDouble(-1) != 9) return 3;
        if (make(5).applyAsDouble(4) != 9 || GLOBAL.applyAsDouble(4) != 7) return 4;
        return 0;
    }
    public static void main(String[] args) {
        if (run() != 0) throw new AssertionError("named SAM storage");
        System.out.println("0");
    }
}
