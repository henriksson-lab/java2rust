import java.util.function.Function;
import java.util.function.BiFunction;
import java.util.function.UnaryOperator;
import java.util.function.BinaryOperator;

public final class GenericCompositionOrderAudit {
    static int state;
    static Function<String, String> argument(boolean fail) {
        state = state * 10 + 2;
        if (fail) throw new IllegalArgumentException("argument");
        return value -> value;
    }
    public static int run() {
        Function<String, String> absent = null;
        try { absent.andThen(argument(false)); return 1; }
        catch (NullPointerException expected) { if (state != 2) return 2; }
        try { absent.compose(argument(true)); return 3; }
        catch (IllegalArgumentException expected) {
            if (state != 22 || !"argument".equals(expected.getMessage())) return 4;
        }
        BiFunction<String, String, int[]> lengths = (a, b) -> new int[] {a.length() + b.length()};
        Function<int[], String> text = value -> String.valueOf(value[0]);
        BiFunction<String, String, String> both = lengths.andThen(text);
        if (!"5".equals(both.apply("ab", "cde"))) return 5;
        UnaryOperator<int[]> increment = value -> { value[0]++; return value; };
        Function<int[], String> incrementText = increment.andThen(text);
        int[] array = new int[] {2};
        if (!"3".equals(incrementText.apply(array)) || array[0] != 3) return 6;
        BinaryOperator<int[]> left = (a, b) -> a;
        BiFunction<int[], int[], String> leftText = left.andThen(text);
        if (!"3".equals(leftText.apply(array, new int[] {99}))) return 7;
        Function<int[], int[]> mutate = increment;
        Function<int[], int[]> twice = mutate.andThen(mutate);
        if (twice == mutate || twice.apply(array) != array || array[0] != 5) return 8;
        Function<int[], int[]> before = mutate.compose(mutate);
        if (before.apply(array) != array || array[0] != 7) return 9;
        Function<int[], int[]> nullable = value -> value;
        if (nullable.apply(null) != null) return 10;
        Function<int[], int[]> narrow = nullable.andThen(value -> value);
        if (narrow.apply(array) != array || array[0] != 7) return 11;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
