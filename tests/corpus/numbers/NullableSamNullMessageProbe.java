import java.util.function.Supplier;
import java.util.function.Function;

public final class NullableSamNullMessageProbe {
    static int invoke(Supplier<Integer> gen) { return gen.get(); }
    static int padded(int padding, Supplier<Integer> gen) { return gen.get() + padding; }
    static int forwarded(Supplier<Integer> gen) { return invoke(gen); }
    static int guarded(boolean enabled, Supplier<Integer> gen) {
        return enabled ? gen.get() : 7;
    }
    static String apply(Function<String, String> fn) { return fn.apply("input"); }
    static int explicit() { throw new NullPointerException("Java null receiver"); }
    public static int run() {
        if (guarded(false, null) != 7) return 1;
        for (int which = 0; which < 5; which++) {
            try {
                switch (which) {
                    case 0: invoke(null); break;
                    case 1: padded(2, null); break;
                    case 2: forwarded(null); break;
                    case 3: apply(null); break;
                    default: invoke(() -> explicit());
                }
                return 10 + which;
            } catch (NullPointerException error) {
                String expected = which == 4 ? "Java null receiver" :
                    which == 3 ? "Cannot invoke \"java.util.function.Function.apply(Object)\" because \"<parameter1>\" is null" :
                    "Cannot invoke \"java.util.function.Supplier.get()\" because \"<parameter" + (which == 1 ? 2 : 1) + ">\" is null";
                if (!expected.equals(error.getMessage())) {
                    System.err.println(which + ": " + error.getMessage());
                    return 20 + which;
                }
            }
        }
        int[] calls = {0};
        Supplier<Integer> stateful = () -> ++calls[0];
        if (invoke(stateful) != 1 || forwarded(stateful) != 2 || calls[0] != 2) return 30;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
