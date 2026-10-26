import java.util.function.IntConsumer;

/** Reference-array element transfers into retained callback composition. */
public final class PrimitiveCompositionArrayAudit {
    private static final IntConsumer EMPTY = new IntConsumer() {
        public void accept(int value) {}
        public IntConsumer andThen(IntConsumer next) { return next; }
    };

    private static IntConsumer compose(IntConsumer... callbacks) {
        IntConsumer result = EMPTY;
        for (IntConsumer callback : callbacks) {
            if (callback != null) result = result.andThen(callback);
        }
        return result == EMPTY ? null : result;
    }

    public static int run() {
        if (compose() != null || compose(null, null) != null) return 1;
        int[] state = {0};
        IntConsumer first = value -> { state[0] += value; };
        IntConsumer second = value -> { state[0] *= 2; };
        if (compose(null, first, null) != first) return 2;
        compose(first, null, second).accept(5);
        if (state[0] != 10) return 3;
        IntConsumer[] callbacks = {first, null, second};
        IntConsumer retained = compose(callbacks);
        callbacks[0] = second;
        state[0] = 0;
        retained.accept(7);
        if (state[0] != 14) return 4;
        IntConsumer failing = value -> {
            state[0] = 31;
            throw new IllegalStateException("first");
        };
        try { compose(failing, second).accept(9); return 5; }
        catch (IllegalStateException expected) {
            if (state[0] != 31 || !"first".equals(expected.getMessage())) return 6;
        }
        try { compose((IntConsumer[]) null); return 7; }
        catch (NullPointerException expected) { if (state[0] != 31) return 8; }
        return 0;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
