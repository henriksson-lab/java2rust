import java.util.function.BiFunction;
import java.util.function.BinaryOperator;
import java.util.function.Function;
import java.util.function.UnaryOperator;

public final class FunctionalParentViewProbe {
    public static int run() {
        UnaryOperator<int[]> operator = values -> { values[0] += 3; return values; };
        Function<int[], int[]> function = operator;
        Function<int[], int[]> again = operator;
        if (function != operator || function != again) return 1;
        int[] input = {4, 7};
        if (function.apply(input) != input || input[0] != 7) return 2;
        if (operator.apply(input) != input || input[0] != 10) return 3;
        if (again.apply(input) != input || input[0] != 13) return 4;
        UnaryOperator<int[]> replacement = values -> { values[0] += 100; return values; };
        if (function == (operator = replacement)) return 10;
        if (function == operator) return 11;

        BinaryOperator<int[]> binary = (left, right) -> { left[1] = right[0]; return left; };
        BiFunction<int[], int[], int[]> binaryView = binary;
        if (binaryView != binary) return 5;
        int[] right = {23};
        if (binaryView.apply(input, right) != input || input[1] != 23) return 6;

        UnaryOperator<int[]> missing = null;
        Function<int[], int[]> nullView = missing;
        if (nullView != null) return 7;
        try {
            nullView.apply(input);
            return 8;
        } catch (NullPointerException expected) {
            if (input[0] != 13) return 9;
        }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
