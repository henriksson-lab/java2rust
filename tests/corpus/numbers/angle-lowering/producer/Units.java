package producer;
import java.util.function.DoubleUnaryOperator;

public class Units {
    public static class Base { public static final double SCALE = 360; }
    public static final class Value extends Base {
        public static final Value ZERO = of(0);
        public static final DoubleUnaryOperator OFFSET = x -> x + 1;
        private final double value;
        private Value(double value) { this.value = value; }
        public static Value of(double value) { return new Value(value); }
        public double read() { return value; }
    }
}
