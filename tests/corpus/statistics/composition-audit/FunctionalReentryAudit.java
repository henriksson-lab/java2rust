import java.util.function.IntUnaryOperator;

public final class FunctionalReentryAudit {
    public static int run() {
        IntUnaryOperator[] callbacks = new IntUnaryOperator[1];
        int[] calls = new int[1];
        callbacks[0] = value -> {
            calls[0]++;
            if (value < 0) throw new IllegalArgumentException("recursive");
            return value == 0 ? 1 : callbacks[0].applyAsInt(value - 1) + 1;
        };
        if (callbacks[0].applyAsInt(3) != 4 || calls[0] != 4) return 1;
        try { callbacks[0].applyAsInt(-1); return 2; }
        catch (IllegalArgumentException expected) {
            if (!"recursive".equals(expected.getMessage())) return 3;
        }
        if (callbacks[0].applyAsInt(1) != 2 || calls[0] != 7) return 4;
        IntUnaryOperator[] composed = new IntUnaryOperator[1];
        IntUnaryOperator base = value -> value == 0 ? 1 : composed[0].applyAsInt(value - 1);
        composed[0] = base.andThen(value -> value + 1);
        if (composed[0].applyAsInt(2) != 4) return 5;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
