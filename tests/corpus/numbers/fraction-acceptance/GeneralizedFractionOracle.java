import java.util.function.Supplier;
import org.apache.commons.numbers.fraction.GeneralizedContinuedFraction;
import org.apache.commons.numbers.fraction.GeneralizedContinuedFraction.Coefficient;

/** Independent public-API caller; this file is never translated. */
public class GeneralizedFractionOracle {
    static String messageUnits(String message) {
        if (message == null) return "NULL";
        StringBuilder units = new StringBuilder();
        for (char unit : message.toCharArray()) units.append(String.format("%04x", (int) unit));
        return units.toString();
    }
    static final class Generator implements Supplier<Coefficient> {
        final int mode;
        int calls;
        Generator(int mode) { this.mode = mode; }
        public Coefficient get() {
            calls++;
            if (calls > 256) throw new IllegalStateException("supplier budget");
            if (mode == 5 && calls == 3) throw new IllegalStateException("supplier stop");
            switch (mode) {
            case 1: return Coefficient.of(calls, calls + 1);
            case 2: return Coefficient.of(1, 0);
            case 3: return Coefficient.of(Double.NaN, 1);
            case 4: return Coefficient.of(1, Double.POSITIVE_INFINITY);
            default: return Coefficient.of(1, 1);
            }
        }
    }
    public static void main(String[] args) {
        double[][] pairs = {{1, 2}, {-0.0, 0.0}, {-1, -2}, {Double.MAX_VALUE, Double.MIN_NORMAL}};
        for (int i = 0; i < pairs.length; i++) {
            Coefficient coefficient = Coefficient.of(pairs[i][0], pairs[i][1]);
            System.out.printf("coefficient:%d\t%016x\t%016x%n", i,
                Double.doubleToRawLongBits(coefficient.getA()), Double.doubleToRawLongBits(coefficient.getB()));
        }
        double[] epsilons = {1e-12, 0, Double.NaN};
        int[] limits = {0, 1, 64};
        for (int overload = 0; overload < 6; overload++) {
            for (int mode = 0; mode < 6; mode++) {
                if (mode >= 2 && mode <= 4 && overload != 2 && overload != 5) continue;
                for (int e = 0; e < epsilons.length; e++) {
                    for (int limit : limits) {
                        Generator generator = new Generator(mode);
                        String label = overload + ":" + mode + ":" + e + ":" + limit;
                        try {
                            double value;
                            switch (overload) {
                            case 0: value = GeneralizedContinuedFraction.value(generator); break;
                            case 1: value = GeneralizedContinuedFraction.value(generator, epsilons[e]); break;
                            case 2: value = GeneralizedContinuedFraction.value(generator, epsilons[e], limit); break;
                            case 3: value = GeneralizedContinuedFraction.value(0, generator); break;
                            case 4: value = GeneralizedContinuedFraction.value(0, generator, epsilons[e]); break;
                            default: value = GeneralizedContinuedFraction.value(0, generator, epsilons[e], limit); break;
                            }
                            System.out.printf("%s\t%016x\t%d%n", label, Double.doubleToRawLongBits(value), generator.calls);
                        } catch (RuntimeException error) {
                            System.out.println(label + "\tERR\t" + error.getClass().getSimpleName()
                                + "\t" + (error instanceof ArithmeticException) + "\t" + messageUnits(error.getMessage()) + "\t" + generator.calls);
                        }
                    }
                }
            }
        }
    }
}
