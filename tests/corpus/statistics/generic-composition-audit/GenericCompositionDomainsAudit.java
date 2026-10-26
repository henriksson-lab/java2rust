import java.util.function.Function;
import java.util.function.Supplier;

/** Contextual generic inference and independent intermediate storage domains. */
public final class GenericCompositionDomainsAudit {
    private static final Function<int[], int[]> SHARED = value -> value;

    public static int run() {
        Function<String, int[]> parse = value -> new int[] {Integer.parseInt(value)};
        Function<int[], String> format = value -> String.valueOf(value[0]);

        Function<String, int[]> increment = parse.andThen(value -> {
            value[0]++;
            return value;
        });
        if (increment.apply("4")[0] != 5) return 1;

        Function<String, String> rebound = parse.andThen(value -> {
            int[] saved = value;
            value = null;
            return String.valueOf(saved[0]);
        });
        if (!"7".equals(rebound.apply("7"))) return 2;

        Function<String, String> before = format.compose(value ->
            new int[] {Integer.parseInt(value)});
        if (!"9".equals(before.apply("9"))) return 3;

        Function<String, Supplier<int[]>> delayed = parse.andThen(value -> () -> value);
        Supplier<int[]> retained = delayed.apply("17");
        int[] same = retained.get();
        same[0] = 18;
        if (retained.get() != same || retained.get()[0] != 18) return 4;

        // A callback already stored in a static field has a different carrier
        // demand from the local lambdas, but the Java array remains one object.
        Function<int[], int[]> shared = SHARED.andThen(value -> value);
        int[] input = {21};
        if (shared.apply(input) != input) return 5;
        input[0] = 22;
        if (shared.apply(input)[0] != 22) return 6;
        return 0;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
