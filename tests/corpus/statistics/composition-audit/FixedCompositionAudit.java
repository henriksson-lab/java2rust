import java.util.function.DoubleConsumer;
import java.util.function.IntConsumer;
import java.util.function.LongConsumer;
import java.util.function.DoubleUnaryOperator;
import java.util.function.IntUnaryOperator;
import java.util.function.LongUnaryOperator;

/** Diagnostic entry gate for the composition families used by Descriptive. */
public final class FixedCompositionAudit {
    private static final DoubleConsumer NOOP = new DoubleConsumer() {
        public void accept(double value) {}
        public DoubleConsumer andThen(DoubleConsumer after) { return after; }
    };
    private static final class Delegate implements IntConsumer {
        int compositions;
        public void accept(int value) { throw new IllegalStateException("unused"); }
        public IntConsumer andThen(IntConsumer after) {
            compositions++;
            return after;
        }
    }
    private static int evaluations;
    private static DoubleConsumer nullReceiver() {
        evaluations = evaluations * 10 + 1;
        return null;
    }
    private static DoubleConsumer argument() {
        evaluations = evaluations * 10 + 2;
        return value -> { evaluations += 100; };
    }
    private static DoubleConsumer throwingArgument() {
        evaluations = evaluations * 10 + 3;
        throw new IllegalArgumentException("argument");
    }
    private static IntConsumer retained(IntConsumer first, IntConsumer second) {
        return first.andThen(second);
    }
    public static int run() {
        int[] trace = {0};
        DoubleConsumer first = value -> {
            trace[0] = trace[0] * 10 + 1;
            if (value < 0) throw new IllegalStateException("first");
        };
        DoubleConsumer second = value -> {
            trace[0] = trace[0] * 10 + 2;
            if (value == 2) throw new IllegalStateException("second");
        };
        DoubleConsumer combined = first.andThen(second);
        if (trace[0] != 0 || combined == first || combined == second) return 1;
        combined.accept(1);
        if (trace[0] != 12) return 2;
        trace[0] = 0;
        first.andThen(first).accept(1);
        if (trace[0] != 11) return 3;
        trace[0] = 0;
        try { first.andThen(null); return 4; }
        catch (NullPointerException expected) {
            if (trace[0] != 0 || expected.getMessage() != null) return 5;
        }
        try { combined.accept(-1); return 6; }
        catch (IllegalStateException expected) {
            if (trace[0] != 1 || !"first".equals(expected.getMessage())) return 7;
        }
        trace[0] = 0;
        try { combined.accept(2); return 8; }
        catch (IllegalStateException expected) {
            if (trace[0] != 12 || !"second".equals(expected.getMessage())) return 9;
        }
        IntConsumer integers = value -> { trace[0] += value; };
        integers.andThen(integers).accept(3);
        if (trace[0] != 18) return 10;
        long[] total = {0};
        LongConsumer longs = value -> { total[0] += value; };
        longs.andThen(longs).accept(3000000000L);
        if (total[0] != 6000000000L) return 11;
        DoubleUnaryOperator add = value -> value + 1;
        DoubleUnaryOperator times = value -> value * 3;
        if (add.compose(times).applyAsDouble(2) != 7) return 12;
        if (add.andThen(times).applyAsDouble(2) != 9) return 13;
        IntUnaryOperator addInt = value -> value + 1;
        IntUnaryOperator timesInt = value -> value * 3;
        if (addInt.compose(timesInt).applyAsInt(2) != 7) return 14;
        if (addInt.andThen(timesInt).applyAsInt(2) != 9) return 15;
        LongUnaryOperator addLong = value -> value + 1;
        LongUnaryOperator timesLong = value -> value * 3;
        if (addLong.compose(timesLong).applyAsLong(3000000000L) != 9000000001L) return 16;
        if (addLong.andThen(timesLong).applyAsLong(3000000000L) != 9000000003L) return 17;
        try { addInt.compose(null); return 26; }
        catch (NullPointerException expected) { if (expected.getMessage() != null) return 27; }
        evaluations = 0;
        try { nullReceiver().andThen(argument()); return 18; }
        catch (NullPointerException expected) { if (evaluations != 12) return 19; }
        evaluations = 0;
        try { nullReceiver().andThen(throwingArgument()); return 20; }
        catch (IllegalArgumentException expected) {
            if (evaluations != 13 || !"argument".equals(expected.getMessage())) return 21;
        }
        IntConsumer escaped = retained(integers, integers);
        trace[0] = 0;
        escaped.accept(7);
        if (trace[0] != 14) return 22;
        if (NOOP.andThen(first) != first || NOOP.andThen(null) != null) return 23;
        Delegate delegate = new Delegate();
        IntConsumer asInterface = delegate;
        if (asInterface.andThen(integers) != integers) return 24;
        if (asInterface.andThen(null) != null || delegate.compositions != 2) return 25;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
