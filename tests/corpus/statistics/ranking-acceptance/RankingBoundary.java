import java.util.function.Function;
import java.util.function.IntUnaryOperator;
import java.util.function.UnaryOperator;
import org.apache.commons.statistics.ranking.NaNStrategy;
import org.apache.commons.statistics.ranking.NaturalRanking;
import org.apache.commons.statistics.ranking.RankingAlgorithm;
import org.apache.commons.statistics.ranking.TiesStrategy;

/** Additional translated API client, kept separate from pinned production. */
public final class RankingBoundary {
    public static int nulls() {
        for (int i = 0; i < 9; i++) {
            try {
                switch (i) {
                case 0: new NaturalRanking((NaNStrategy) null); break;
                case 1: new NaturalRanking((TiesStrategy) null); break;
                case 2: new NaturalRanking((IntUnaryOperator) null); break;
                case 3: new NaturalRanking(null, TiesStrategy.AVERAGE); break;
                case 4: new NaturalRanking(NaNStrategy.FAILED, (TiesStrategy) null); break;
                case 5: new NaturalRanking((NaNStrategy) null, (TiesStrategy) null); break;
                case 6: new NaturalRanking(null, (IntUnaryOperator) null); break;
                case 7: new NaturalRanking(NaNStrategy.FAILED, (IntUnaryOperator) null); break;
                default: new NaturalRanking().apply(null); break;
                }
                return i + 1;
            } catch (NullPointerException expected) {
                if (i < 8) {
                    String message = i == 0 || i == 3 || i == 5 || i == 6 ? "nanStrategy" :
                        i == 1 || i == 4 ? "tiesStrategy" : "randomIntFunction";
                    if (!message.equals(expected.getMessage())) return 20 + i;
                }
            }
        }
        return 0;
    }
    public static int inherited() {
        NaturalRanking ranking = new NaturalRanking();
        RankingAlgorithm algorithm = ranking;
        UnaryOperator<double[]> operator = algorithm;
        Function<double[], double[]> function = operator;
        double[] input = {3, 1, 2};
        double[] a = algorithm.apply(input);
        double[] b = operator.apply(input);
        double[] c = function.apply(input);
        if (a[0] != 3 || a[1] != 1 || a[2] != 2) return 1;
        if (b[0] != 3 || b[1] != 1 || b[2] != 2) return 2;
        if (c[0] != 3 || c[1] != 1 || c[2] != 2) return 3;
        Function<double[], double[]> before = function.compose(x -> x);
        Function<double[], double[]> after = function.andThen(x -> x);
        if (before.apply(input)[0] != 3 || after.apply(input)[1] != 1) return 4;
        if (input[0] != 3 || input[1] != 1 || input[2] != 2) return 5;
        return 0;
    }
    public static int run() {
        int errors = nulls();
        return errors != 0 ? errors : inherited();
    }
    public static void main(String[] args) { System.out.println(run()); }
}
