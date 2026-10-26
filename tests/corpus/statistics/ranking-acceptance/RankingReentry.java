import java.util.function.IntUnaryOperator;
import org.apache.commons.statistics.ranking.NaturalRanking;

/** Recursive callbacks exercise invocation guards, not concurrent ranking. */
public final class RankingReentry {
    public static int primitive() {
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
        return 0;
    }
    public static int ranking() {
        NaturalRanking[] holder = new NaturalRanking[1];
        boolean[] nested = new boolean[1];
        int[] calls = new int[1];
        IntUnaryOperator random = bound -> {
            calls[0]++;
            if (!nested[0]) {
                nested[0] = true;
                double[] result = holder[0].apply(new double[] {2, 2});
                nested[0] = false;
                if (result[0] + result[1] != 3 || result[0] == result[1]) {
                    throw new IllegalStateException("nested ranks");
                }
            }
            return 0;
        };
        holder[0] = new NaturalRanking(random);
        double[] input = new double[] {5, 5, 5};
        double[] result = holder[0].apply(input);
        if (calls[0] != 4) return 1;
        if (result[0] + result[1] + result[2] != 6) return 2;
        if (result[0] == result[1] || result[0] == result[2] || result[1] == result[2]) return 3;
        if (input[0] != 5 || input[1] != 5 || input[2] != 5) return 4;
        return 0;
    }
    public static void main(String[] args) {
        System.out.println("primitive".equals(args[0]) ? primitive() : ranking());
    }
}
