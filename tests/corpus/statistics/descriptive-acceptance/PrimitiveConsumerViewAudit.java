package consumerview;
import java.util.function.DoubleConsumer;
import java.util.function.IntConsumer;
import java.util.function.LongConsumer;
interface Value extends java.util.function.DoubleSupplier, java.util.function.LongSupplier {
    double getAsDouble();
    default long getAsLong() { return (long)getAsDouble(); }
}
interface DoubleState extends DoubleConsumer, Value {}
class DoubleCell implements DoubleState {
    double value;
    int calls;
    public void accept(double x) { calls++; if (x < 0) throw new IllegalArgumentException("negative"); value += x; }
    public double getAsDouble() { return value; }
}
class IntCell implements IntConsumer {
    int value;
    public void accept(int x) { value += x; }
}
class LongCell implements LongConsumer {
    long value;
    public void accept(long x) { value += x; }
}
public class PrimitiveConsumerViewAudit {
    static DoubleConsumer[] retained;
    static DoubleConsumer view(DoubleState value) { return value; }
    static DoubleConsumer compose(DoubleConsumer... values) {
        DoubleConsumer action = x -> {};
        for (DoubleConsumer value : values) if (value != null) action = action.andThen(value);
        return action;
    }
    static IntConsumer composeInts(IntConsumer... values) {
        IntConsumer action = x -> {};
        for (IntConsumer value : values) if (value != null) action = action.andThen(value);
        return action;
    }
    static LongConsumer composeLongs(LongConsumer... values) {
        LongConsumer action = x -> {};
        for (LongConsumer value : values) if (value != null) action = action.andThen(value);
        return action;
    }
    public static int run() {
        DoubleCell first = new DoubleCell();
        DoubleCell second = new DoubleCell();
        DoubleConsumer a = first;
        DoubleConsumer b = view(first);
        if (a != b || (Object)a != (Object)first) return 1;
        retained = new DoubleConsumer[] {a, second};
        DoubleConsumer explicit = compose(retained);
        explicit.accept(0);
        DoubleConsumer both = compose(retained[0], null, retained[1]);
        both.accept(3.5);
        if (first.getAsDouble() != 3.5 || second.value != 3.5 || first.calls != 2) return 2;
        first.accept(2);
        b.accept(1);
        if (first.value != 6.5 || first.getAsLong() != 6 || first.calls != 4) return 3;
        try { both.accept(-1); return 4; }
        catch (IllegalArgumentException e) { if (!"negative".equals(e.getMessage())) return 5; }
        if (first.calls != 5 || second.calls != 2 || first.value != 6.5) return 6;
        IntCell ints = new IntCell();
        IntConsumer ic = composeInts(ints, null, ints);
        ic.accept(7);
        if (ints.value != 14) return 7;
        LongCell longs = new LongCell();
        LongConsumer lc = composeLongs(longs, null);
        lc.accept(3000000000L);
        if (longs.value != 3000000000L) return 8;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
