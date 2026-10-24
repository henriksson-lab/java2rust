import java.math.BigInteger;

/** Independent caller: compiled by javac, never part of translation input. */
public class CanonicalMappedFieldsCaller {
    public static int run() {
        BigInteger huge = BigInteger.valueOf(2).pow(100);
        CanonicalMappedFieldsProbe left = new CanonicalMappedFieldsProbe(huge, BigInteger.valueOf(3));
        CanonicalMappedFieldsProbe right = new CanonicalMappedFieldsProbe(BigInteger.valueOf(5), BigInteger.valueOf(7));
        for (int i = 0; i < 3; ++i) {
            CanonicalMappedFieldsProbe product = left.multiply(right);
            if (product.numerator.compareTo(huge.multiply(BigInteger.valueOf(5))) != 0) return 1;
            if (product.denominator.intValue() != 21) return 2;
            if (left.multiply(2).numerator.compareTo(huge.add(huge)) != 0) return 3;
            if (left.multiply(BigInteger.valueOf(2)).numerator.compareTo(huge.add(huge)) != 0) return 4;
        }
        if (left.identity() != left || CanonicalMappedFieldsProbe.ZERO.identity() != CanonicalMappedFieldsProbe.ZERO) return 5;
        if (CanonicalMappedFieldsProbe.ZERO.multiply(right).numerator.signum() != 0) return 6;
        if (left.numerator.compareTo(huge) != 0 || left.denominator.intValue() != 3) return 7;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
