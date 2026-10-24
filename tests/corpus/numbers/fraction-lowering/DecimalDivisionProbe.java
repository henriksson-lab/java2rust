package decimaldivision;

import java.math.BigDecimal;
import java.math.BigInteger;
import java.math.RoundingMode;

/** All rational-to-decimal work goes through JDK/generated runtime APIs. */
public class DecimalDivisionProbe {
    public static int run() {
        BigInteger huge = BigInteger.valueOf(2).pow(100);
        BigDecimal wide = new BigDecimal(huge);
        if (!wide.toString().equals("1267650600228229401496703205376") || wide.scale() != 0) return 1;
        if (huge.bitLength() != 101) return 2;
        BigDecimal left = new BigDecimal("1.00");
        BigDecimal right = new BigDecimal(2);
        BigDecimal exact = left.divide(right);
        if (!exact.toString().equals("0.50") || exact.scale() != 2) return 3;
        if (!new BigDecimal(1).divide(new BigDecimal(8)).toString().equals("0.125")) return 4;
        if (!left.divide(new BigDecimal(3), RoundingMode.HALF_UP).toString().equals("0.33")) return 5;
        if (!left.divide(new BigDecimal(3), 4, RoundingMode.HALF_UP).toString().equals("0.3333")) return 6;
        if (!new BigDecimal(-5).divide(right, RoundingMode.FLOOR).toString().equals("-3")) return 7;
        if (!new BigDecimal(5).divide(right, RoundingMode.HALF_EVEN).toString().equals("2")) return 8;
        if (!wide.divide(new BigDecimal(4)).toString().equals("316912650057057350374175801344")) return 9;
        if (!left.toString().equals("1.00") || !right.toString().equals("2")
                || !left.divide(right).toString().equals("0.50")) return 10;
        try { left.divide(new BigDecimal(3)); return 11; }
        catch (ArithmeticException error) {
            if (!"Non-terminating decimal expansion; no exact representable decimal result.".equals(error.getMessage())) return 12;
        }
        try { left.divide(new BigDecimal(3), 4, RoundingMode.UNNECESSARY); return 13; }
        catch (ArithmeticException error) {
            if (!"Rounding necessary".equals(error.getMessage())) return 14;
        }
        try { left.divide(new BigDecimal(0)); return 15; }
        catch (ArithmeticException error) {
            if (!"Division by zero".equals(error.getMessage())) return 16;
        }
        try { new BigDecimal(0).divide(new BigDecimal(0)); return 17; }
        catch (ArithmeticException error) {
            if (!"Division undefined".equals(error.getMessage())) return 18;
        }
        try { left.divide((BigDecimal)null); return 19; }
        catch (NullPointerException expected) { }
        try { left.divide(right, (RoundingMode)null); return 20; }
        catch (NullPointerException expected) { }
        try { left.divide(right, 2, (RoundingMode)null); return 21; }
        catch (NullPointerException expected) { }
        return 0;
    }
    public static void main(String[] args) {
        int result = run();
        if (result != 0) throw new AssertionError("decimal division contract: " + result);
        System.out.println(result);
    }
}
