package integerarithmetic;

import java.math.BigInteger;

/** Runtime contract; no rational arithmetic is reimplemented here. */
public class BigIntegerArithmeticProbe {
    public static int run() {
        BigInteger huge = BigInteger.valueOf(2).pow(200);
        BigInteger divisor = BigInteger.valueOf(2).pow(100);
        if (huge.divide(divisor).compareTo(divisor) != 0) return 1;
        if (huge.add(huge).subtract(huge).compareTo(huge) != 0) return 2;
        if (huge.gcd(divisor.negate()).compareTo(divisor) != 0) return 3;
        if (BigInteger.valueOf(-7).divide(BigInteger.valueOf(3)).intValue() != -2
                || BigInteger.valueOf(7).divide(BigInteger.valueOf(-3)).intValue() != -2) return 4;
        if (huge.signum() != 1 || huge.negate().signum() != -1
                || huge.negate().abs().compareTo(huge) != 0 || BigInteger.ZERO.signum() != 0) return 5;
        if (BigInteger.ZERO.bitLength() != 0 || BigInteger.valueOf(-1).bitLength() != 0
                || BigInteger.valueOf(-8).bitLength() != 3 || huge.bitLength() != 201) return 6;
        if (BigInteger.ZERO.getLowestSetBit() != -1 || divisor.getLowestSetBit() != 100
                || BigInteger.valueOf(-8).getLowestSetBit() != 3) return 7;
        BigInteger changed = BigInteger.valueOf(-1).flipBit(65);
        if (changed.testBit(65) || changed.flipBit(65).compareTo(BigInteger.valueOf(-1)) != 0) return 8;
        if (BigInteger.valueOf(4294967296L).hashCode() != 31
                || BigInteger.valueOf(-4294967296L).hashCode() != -31) return 9;
        for (int i = 0; i < 3; i++) {
            if (huge.subtract(divisor).add(divisor).compareTo(huge) != 0) return 10;
        }
        try { huge.divide(BigInteger.ZERO); return 11; }
        catch (ArithmeticException expected) {
            if (!"BigInteger divide by zero".equals(expected.getMessage())) return 12;
        }
        try { huge.flipBit(-1); return 13; }
        catch (ArithmeticException expected) {
            if (!"Negative bit address".equals(expected.getMessage())) return 14;
        }
        try { huge.add((BigInteger)null); return 15; }
        catch (NullPointerException expected) { }
        if (huge.bitLength() != 201 || divisor.bitLength() != 101) return 16;
        return 0;
    }
    public static void main(String[] args) {
        int result = run();
        if (result != 0) throw new AssertionError("integer contract: " + result);
        System.out.println(result);
    }
}
