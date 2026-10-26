import java.util.function.Function;
import java.util.function.Supplier;

/** Diagnostic entry gate for callback-local value domains. */
public final class FunctionalLambdaDomainsAudit {
    static int calls;
    static int[] input = {91};
    static int[] update(int[] input) {
        calls++;
        input[0] += 4;
        return input;
    }
    static Function<int[], int[]> identity() { return input -> input; }
    static Function<int[], int[]> forwarding() { return input -> update(input); }
    static Function<int[], int[]> alias() {
        return input -> { int[] saved = input; return saved; };
    }
    static Function<int[], int[]> rebound() {
        return input -> { input = null; return input; };
    }
    static Function<int[], int[]> localRebound() {
        return input -> { int[] saved = input; saved = null; return saved; };
    }
    static Function<int[], Supplier<int[]>> delayed() { return input -> () -> input; }

    public static int run() {
        Function<int[], int[]> identity = identity();
        int[] input = {3};
        if (identity.apply(input) != input || input[0] != 3) return 1;
        if (identity.apply(null) != null) return 2;
        Function<int[], int[]> forwarding = forwarding();
        if (forwarding.apply(input) != input || input[0] != 7 || calls != 1) return 3;
        try {
            forwarding.apply(null);
            return 4;
        } catch (NullPointerException expected) {
            if (calls != 2 || input[0] != 7) return 5;
        }
        Function<int[], int[]> alias = alias();
        if (alias.apply(input) != input || alias.apply(null) != null) return 6;
        if (rebound().apply(input) != null || localRebound().apply(input) != null) return 7;
        if (input[0] != 7 || FunctionalLambdaDomainsAudit.input[0] != 91) return 8;
        Function<int[], Supplier<int[]>> delayed = delayed();
        Supplier<int[]> retained = delayed.apply(input);
        input[0] = 17;
        if (retained.get() != input || retained.get()[0] != 17) return 9;
        if (delayed.apply(null).get() != null) return 10;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
