import java.util.function.Function;

public final class FunctionalArrayDomainProbe {
    static final class Mutator implements Function<int[], int[]> {
        private int[] last;
        public int[] apply(int[] input) {
            input[0] += 3;
            last = input;
            return input;
        }
        int[] last() { return last; }
    }

    static Function<int[], int[]> retain(Mutator mutator) { return mutator; }

    public static int run() {
        Mutator mutator = new Mutator();
        Function<int[], int[]> function = retain(mutator);
        Function<int[], int[]> alias = function;
        int[] input = {4, 8};
        int[] output = alias.apply(input);
        if (output != input || mutator.last() != input || input[0] != 7) return 1;
        input[1] = 19;
        if (output[1] != 19 || function.apply(output) != input || input[0] != 10) return 2;
        int[] other = {1, 2};
        if (function.apply(other) != other || mutator.last() != other) return 3;
        if (input[0] != 10 || other[0] != 4) return 4;
        try {
            function.apply(null);
            return 5;
        } catch (NullPointerException expected) {
            if (mutator.last() != other) return 6;
        }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
