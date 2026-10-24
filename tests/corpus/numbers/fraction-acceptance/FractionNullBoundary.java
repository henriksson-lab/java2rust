import java.math.BigInteger;
import java.math.RoundingMode;
import java.util.function.Supplier;
import org.apache.commons.numbers.fraction.Fraction;
import org.apache.commons.numbers.fraction.BigFraction;
import org.apache.commons.numbers.fraction.GeneralizedContinuedFraction;
import org.apache.commons.numbers.fraction.GeneralizedContinuedFraction.Coefficient;

/** Caller-side null contracts, co-translated to make nullable call sites explicit. */
public final class FractionNullBoundary {
    public static int check(int which) {
        Fraction fraction=Fraction.of(2,3);
        BigFraction big=BigFraction.of(2,3);
        try {
            switch (which) {
            case 0: Fraction.parse(null); break;
            case 1: BigFraction.parse(null); break;
            case 2: BigFraction.of((BigInteger)null); break;
            case 3: BigFraction.of(BigInteger.ONE,(BigInteger)null); break;
            case 4: BigFraction.of((BigInteger)null,BigInteger.ONE); break;
            case 5: big.add((BigInteger)null); break;
            case 6: big.subtract((BigInteger)null); break;
            case 7: big.multiply((BigInteger)null); break;
            case 8: big.divide((BigInteger)null); break;
            case 9: big.add((BigFraction)null); break;
            case 10: big.subtract((BigFraction)null); break;
            case 11: big.multiply((BigFraction)null); break;
            case 12: big.divide((BigFraction)null); break;
            case 13: fraction.add((Fraction)null); break;
            case 14: fraction.subtract((Fraction)null); break;
            case 15: fraction.multiply((Fraction)null); break;
            case 16: fraction.divide((Fraction)null); break;
            case 17: fraction.compareTo(null); break;
            case 18: big.compareTo(null); break;
            case 19: big.bigDecimalValue((RoundingMode)null); break;
            case 20: big.bigDecimalValue(3,(RoundingMode)null); break;
            case 21: GeneralizedContinuedFraction.value((Supplier<Coefficient>)null); break;
            case 22: GeneralizedContinuedFraction.value(1.0,(Supplier<Coefficient>)null); break;
            default: throw new IllegalArgumentException("unknown boundary");
            }
        } catch (NullPointerException expected) {
            return 0;
        }
        return 1;
    }
    public static int compare(int which) {
        int otherNumerator=which%3==0 ? -2 : which%3==1 ? 2 : 3;
        if (which<3) {
            Comparable<Fraction> value=Fraction.of(2,3);
            return value.compareTo(Fraction.of(otherNumerator,3));
        }
        Comparable<BigFraction> value=BigFraction.of(2,3);
        return value.compareTo(BigFraction.of(otherNumerator,3));
    }
    public static double invokeSupplier(int which) {
        if (which == 0) return GeneralizedContinuedFraction.value((Supplier<Coefficient>)null);
        return GeneralizedContinuedFraction.value(1.0,(Supplier<Coefficient>)null);
    }
    public static void main(String[] args) {
        for (int which=0;which<23;which++) {
            System.out.println(which+"\t"+check(which));
        }
    }
}
