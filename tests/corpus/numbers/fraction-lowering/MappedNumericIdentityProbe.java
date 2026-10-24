package mappednumericidentity;

import java.math.BigInteger;
import java.math.BigDecimal;
import java.math.RoundingMode;

/** The immutable mapped-field/getter contract used by BigFraction. */
public class MappedNumericIdentityProbe {
    static int observations;
    static BigInteger observed(BigInteger value) { observations++; return value; }
    static final class NumericHolder {
        final BigInteger value;
        NumericHolder(BigInteger value) { this.value = value; }
        BigInteger read() { return value; }
    }
    static final class DecimalHolder {
        final BigDecimal value;
        DecimalHolder(BigDecimal value) { this.value = value; }
        BigDecimal read() { return value; }
    }
    public static int run() {
        BigInteger input = new BigInteger("123456789012345678901234567890");
        NumericHolder holder = new NumericHolder(input);
        if (holder.read() != input) return 1;
        if (holder.read() != holder.read()) return 2;
        BigInteger equal = new BigInteger("123456789012345678901234567890");
        if (equal == input) return 3;
        if (!equal.equals(input)) return 4;
        BigInteger alias = input;
        if (alias != input) return 5;
        NumericHolder absent = new NumericHolder(null);
        if (absent.read() != null || absent.read() != absent.read()) return 6;
        if (input != holder.read() || input == absent.read() || absent.read() == input) return 7;
        NumericHolder different = new NumericHolder(equal);
        if (holder.read() == different.read()) return 8;
        if (absent.read() == observed(input) || observations != 1) return 9;
        if (absent.read() != observed(null) || observations != 2) return 10;
        BigInteger zero = new BigInteger("0");
        BigInteger anotherZero = new BigInteger("0");
        if (zero == anotherZero || zero == BigInteger.ZERO || !zero.equals(BigInteger.ZERO)) return 11;
        if (BigInteger.valueOf(1) != BigInteger.ONE || input.abs() != input) return 12;
        BigDecimal decimal = new BigDecimal("1.00");
        BigDecimal sameValue = new BigDecimal("1.00");
        DecimalHolder decimalHolder = new DecimalHolder(decimal);
        if (decimalHolder.read() != decimal || decimalHolder.read() != decimalHolder.read()) return 13;
        if (decimal == sameValue || !decimal.equals(sameValue)) return 14;
        if (decimal.setScale(2, RoundingMode.UNNECESSARY) != decimal) return 15;
        return 0;
    }
    public static void main(String[] args) {
        int result = run();
        if (result != 0) throw new AssertionError("mapped identity: " + result);
        System.out.println(result);
    }
}
