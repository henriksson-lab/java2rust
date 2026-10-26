package consumerbound;
import java.util.function.DoubleConsumer;
import java.util.function.IntConsumer;
import java.util.function.LongConsumer;
interface Result { double value(); }
interface DoubleState extends DoubleConsumer, Result {}
class DoubleCell implements DoubleState {
    double sum;
    int calls;
    public void accept(double v) {
        calls++;
        if (v < 0) throw new IllegalArgumentException("negative");
        if (v == 42) GenericConsumerBoundAudit.feed(this, new double[] {0.25});
        sum += v;
    }
    public double value() { return sum; }
}
class IntCell implements IntConsumer {
    int sum;
    public void accept(int v) { sum += v; }
}
class LongCell implements LongConsumer {
    long sum;
    public void accept(long v) { sum += v; }
}
class EphemeralInt implements IntConsumer {
    int sum;
    public void accept(int v) { sum += v; }
}
public class GenericConsumerBoundAudit {
    static DoubleConsumer stored;
    static int argumentCalls;
    static double next() { argumentCalls++; return 1; }
    static <T extends DoubleConsumer> void one(T statistic) { statistic.accept(next()); }
    static <T extends DoubleConsumer> T feed(T statistic, double[] values) {
        for (double value : values) statistic.accept(value);
        return statistic;
    }
    static <T extends IntConsumer> T feedInt(T statistic, int[] values) {
        for (int value : values) statistic.accept(value);
        return statistic;
    }
    static <T extends LongConsumer> T feedLong(T statistic, long[] values) {
        for (long value : values) statistic.accept(value);
        return statistic;
    }
    public static int run() {
        DoubleCell cell = new DoubleCell();
        if (feed(cell, new double[] {1, 2.5}) != cell || cell.sum != 3.5 || cell.calls != 2) return 1;
        DoubleState view = cell;
        if (feed(view, new double[] {0.5}) != view || cell.value() != 4 || cell.calls != 3) return 2;
        try { feed(view, new double[] {1, -1, 100}); return 3; }
        catch (IllegalArgumentException e) { if (!"negative".equals(e.getMessage())) return 4; }
        if (cell.sum != 5 || cell.calls != 5) return 5;
        final double[] sum = {0};
        DoubleConsumer local = v -> { sum[0] += v; };
        if (feed(local, new double[] {2}) != local || sum[0] != 2) return 6;
        stored = v -> { sum[0] += v; };
        if (feed(stored, new double[] {3}) != stored || sum[0] != 5) return 7;
        if (feed((DoubleConsumer)null, new double[0]) != null) return 8;
        try { feed((DoubleConsumer)null, new double[] {1}); return 9; }
        catch (NullPointerException e) { }
        IntCell ints = new IntCell();
        if (feedInt(ints, new int[] {2, 3}) != ints || ints.sum != 5) return 10;
        LongCell longs = new LongCell();
        if (feedLong(longs, new long[] {3000000000L, 2}) != longs || longs.sum != 3000000002L) return 11;
        feed(cell, new double[] {42});
        if (cell.sum != 47.25 || cell.calls != 7) return 12;
        DoubleConsumer checked = v -> {
            if (v < 0) throw new IllegalArgumentException("callback");
            sum[0] += v;
        };
        try { feed(checked, new double[] {1, -1, 100}); return 13; }
        catch (IllegalArgumentException e) { if (!"callback".equals(e.getMessage())) return 14; }
        if (sum[0] != 6) return 15;
        try { one((DoubleConsumer)null); return 16; }
        catch (NullPointerException e) { }
        if (argumentCalls != 1) return 17;
        if (feedInt(new EphemeralInt(), new int[] {2, 7}).sum != 9) return 18;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
