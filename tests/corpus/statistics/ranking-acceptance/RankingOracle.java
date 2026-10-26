import java.util.Arrays;
import org.apache.commons.statistics.ranking.*;

/** Independent caller: invokes production ranking; never implements ranking. */
public final class RankingOracle {
    static double[] data(int k) {
        switch (k) {
        case 0: return new double[0];
        case 1: return new double[]{4};
        case 2: return new double[]{3, 1, 2, 1};
        case 3: return new double[]{-0.0, 0.0, -0.0, 0.0};
        case 4: return new double[]{Double.longBitsToDouble(0x7ff8000000000042L), Double.longBitsToDouble(0xfff8000000000043L)};
        case 5: return new double[]{Double.NaN, 3, 1, 3, Double.NaN};
        case 6: return new double[]{Double.NEGATIVE_INFINITY, Double.POSITIVE_INFINITY, 2, 2, Double.NaN};
        default:
            double[] x = new double[40];
            for (int i = 0; i < x.length; i++) x[i] = i % 3;
            return x;
        }
    }
    static String bits(double[] values) {
        StringBuilder s = new StringBuilder();
        for (double v : values) { if (s.length() != 0) s.append(','); s.append(Long.toHexString(Double.doubleToRawLongBits(v))); }
        return s.toString();
    }
    static void row(String label, NaturalRanking rank, double[] input) {
        String before = bits(input);
        String status = "OK", output = "-";
        boolean alias = false;
        try {
            double[] result = rank.apply(input);
            alias = result == input;
            output = bits(result);
        } catch (RuntimeException error) {
            StringBuilder message = new StringBuilder();
            String text = error.getMessage();
            if (text == null) message.append("NULL");
            else for (int i = 0; i < text.length(); i++) message.append(String.format("%04x", (int) text.charAt(i)));
            status = error.getClass().getName() + ":" + message;
        }
        System.out.println(label + "\t" + status + "\t" + output + "\t" + before.equals(bits(input)) + "\t" + alias + "\t" + rank.getNanStrategy().name() + "\t" + rank.getTiesStrategy().name());
    }
    public static void main(String[] args) {
        for (NaNStrategy nan : NaNStrategy.values()) {
            for (TiesStrategy ties : TiesStrategy.values()) {
                NaturalRanking rank = ties == TiesStrategy.RANDOM ? new NaturalRanking(nan, b -> 0) : new NaturalRanking(nan, ties);
                for (int k = 0; k < 8; k++) row(nan.name() + ":" + ties.name() + ":" + k, rank, data(k));
            }
        }
        NaturalRanking[] constructors = {
            new NaturalRanking(), new NaturalRanking(NaNStrategy.MINIMAL),
            new NaturalRanking(TiesStrategy.MINIMUM), new NaturalRanking(NaNStrategy.FIXED, TiesStrategy.SEQUENTIAL),
            new NaturalRanking(b -> 0), new NaturalRanking(NaNStrategy.MINIMAL, b -> 0),
        };
        for (int i = 0; i < constructors.length; i++) row("ctor:" + i, constructors[i], new double[]{5, 2, 2});
        NaturalRanking random = new NaturalRanking(TiesStrategy.RANDOM);
        boolean valid = true;
        for (int repeat = 0; repeat < 16; repeat++) {
            double[] input = new double[8];
            Arrays.fill(input, 3);
            double[] ranks = random.apply(input);
            Arrays.sort(ranks);
            for (int i = 0; i < 8; i++) valid &= ranks[i] == i + 1 && input[i] == 3;
        }
        System.out.println("native-random\t" + valid);
    }
}
