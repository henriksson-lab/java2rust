import java.math.BigInteger;
import static java.math.BigInteger.ONE;

/** Independent JDK observations; also translated unchanged as an API probe. */
public final class BigIntegerOracle {
    private static final BigInteger SAVED_ONE = ONE;
    public static String run() {
        StringBuilder out = new StringBuilder();
        long[] bases = {0, 1, -1, 2, -2, 3, -3, 123456789, Long.MIN_VALUE, Long.MAX_VALUE};
        int[] exponents = {0, 1, 2, 3, 7, 20, -1};
        int[] shifts = {0, 1, 31, 63, 64, 127, 1000, Integer.MAX_VALUE, -1, -31, -65, -130, Integer.MIN_VALUE};
        int[] bits = {0, 1, 2, 31, 32, 63, 64, 127, 1000, Integer.MAX_VALUE, -1, Integer.MIN_VALUE};
        out.append("constants=").append(BigInteger.ZERO.toString()).append(",").append(ONE.toString()).append(",").append(SAVED_ONE.toString()).append("\n");
        {
            BigInteger ONE = BigInteger.ZERO;
            out.append("local=").append(ONE.toString()).append("\n");
        }
        for (long base : bases) {
            BigInteger value = BigInteger.valueOf(base);
            for (int exponent : exponents) {
                out.append("pow:").append(base).append(":").append(exponent).append("=");
                try { out.append(value.pow(exponent).toString()); }
                catch (ArithmeticException error) { out.append(error.getMessage()); }
                out.append("\n");
            }
            for (int shift : shifts) {
                out.append("shift:").append(base).append(":").append(shift).append("=");
                try { out.append(value.shiftRight(shift).toString()); }
                catch (ArithmeticException error) { out.append(error.getMessage()); }
                out.append("\n");
            }
            for (int bit : bits) {
                out.append("bit:").append(base).append(":").append(bit).append("=");
                try { out.append(value.testBit(bit)); }
                catch (ArithmeticException error) { out.append(error.getMessage()); }
                out.append("\n");
            }
            for (long other : bases) {
                BigInteger right = BigInteger.valueOf(other);
                out.append("multiply:").append(base).append(":").append(other).append("=").append(value.multiply(right).toString()).append("\n");
                out.append("compare:").append(base).append(":").append(other).append("=").append(value.compareTo(right)).append("\n");
            }
            out.append("unchanged:").append(base).append("=").append(value.toString()).append("\n");
        }
        BigInteger huge = BigInteger.valueOf(-123456789).pow(20);
        BigInteger product = huge.multiply(huge);
        out.append("huge=").append(huge.toString()).append("\n");
        out.append("product=").append(product.toString()).append("\n");
        out.append("wide_shift=").append(huge.shiftRight(-211).toString()).append("\n");
        out.append("wide_bit=").append(product.testBit(1000)).append("\n");
        out.append("huge_unchanged=").append(huge.toString()).append("\n");
        for (int base = -2; base <= 2; base++) {
            out.append("extreme_pow:").append(base).append("=");
            try { out.append(BigInteger.valueOf(base).pow(Integer.MAX_VALUE).toString()); }
            catch (ArithmeticException error) { out.append(error.getMessage()); }
            out.append("\n");
        }
        return out.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
