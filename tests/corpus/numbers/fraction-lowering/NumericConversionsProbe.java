package numericconversions;
import java.math.BigInteger;
import java.math.BigDecimal;
class Retained {
    BigInteger value;
    Retained(BigInteger value) { this.value = value; }
    BigInteger get() { return value; }
    double approximate() { return value.doubleValue(); }
    long exact() { return value.longValueExact(); }
}
public class NumericConversionsProbe {
    static int trace;
    static BigInteger active;
    static int replace() { active = BigInteger.valueOf(11); return 1; }
    static BigInteger receiver(BigInteger value) { trace = trace * 10 + 1; return value; }
    static int distance() { trace = trace * 10 + 2; return 4; }
    static int badDistance() { trace = trace * 10 + 3; throw new IllegalStateException("argument"); }
    public static int run() {
        BigInteger maxInt = new BigInteger("2147483647");
        BigInteger minInt = new BigInteger("-2147483648");
        if (maxInt.intValueExact() != Integer.MAX_VALUE || minInt.intValueExact() != Integer.MIN_VALUE) return 1;
        BigInteger maxLong = new BigInteger("9223372036854775807");
        BigInteger minLong = new BigInteger("-9223372036854775808");
        if (maxLong.longValueExact() != Long.MAX_VALUE || minLong.longValueExact() != Long.MIN_VALUE) return 2;
        try { maxInt.add(BigInteger.ONE).intValueExact(); return 3; }
        catch (ArithmeticException e) { if (!"BigInteger out of int range".equals(e.getMessage())) return 4; }
        try { minInt.subtract(BigInteger.ONE).intValueExact(); return 5; }
        catch (ArithmeticException e) { }
        try { maxLong.add(BigInteger.ONE).longValueExact(); return 6; }
        catch (ArithmeticException e) { if (!"BigInteger out of long range".equals(e.getMessage())) return 7; }
        try { minLong.subtract(BigInteger.ONE).longValueExact(); return 8; }
        catch (ArithmeticException e) { }
        BigInteger wide = BigInteger.valueOf(3).shiftLeft(64);
        if (!"55340232221128654848".equals(wide.toString())) return 9;
        if (wide.shiftLeft(-64).longValueExact() != 3) return 10;
        if (BigInteger.valueOf(-9).shiftLeft(-1).intValueExact() != -5) return 11;
        if (BigInteger.valueOf(7).shiftLeft(0).intValueExact() != 7) return 12;
        if (BigInteger.valueOf(-3).shiftRight(-4).longValueExact() != -48) return 13;
        long[] values = {0, 1, -1, 9007199254740991L, 9007199254740992L, 9007199254740993L, 9007199254740995L, Long.MAX_VALUE, Long.MIN_VALUE};
        for (long value : values) {
            if (Double.doubleToLongBits(BigInteger.valueOf(value).doubleValue()) != Double.doubleToLongBits((double)value)) return 14;
        }
        if (Double.doubleToLongBits(new BigInteger("18446744073709553665").doubleValue()) != Double.doubleToLongBits(18446744073709555712.0)) return 15;
        Retained kept = new Retained(BigInteger.valueOf(23));
        BigInteger alias = kept.get();
        if (kept.exact() != 23 || kept.approximate() != 23.0 || alias != kept.get()) return 16;
        BigInteger direct = kept.value;
        if (direct != alias || kept.exact() != 23) return 28;
        if (alias.shiftLeft(4).intValueExact() != 368 || kept.exact() != 23) return 17;
        if (!"-123".equals(new BigDecimal("-123.999").toBigInteger().toString())) return 18;
        if (!"123".equals(new BigDecimal("123.001").toBigInteger().toString())) return 19;
        if (!"123000".equals(new BigDecimal("1.23E+5").toBigInteger().toString())) return 20;
        if (new BigDecimal("-0.25").toBigInteger().intValueExact() != 0) return 21;
        if (new BigDecimal("0E+200").toBigInteger().intValueExact() != 0) return 22;
        if (new BigDecimal("1E-324").toBigInteger().intValueExact() != 0) return 23;
        if (receiver(BigInteger.ONE).shiftLeft(distance()).intValueExact() != 16 || trace != 12) return 24;
        trace = 0;
        try { receiver(null).shiftLeft(distance()); return 25; }
        catch (NullPointerException e) { }
        if (trace != 12) return 26;
        trace = 0;
        try { if (receiver(null).shiftLeft(distance()).intValueExact() != 0) return 29; return 30; }
        catch (NullPointerException e) { }
        if (trace != 12) return 31;
        trace = 0;
        try { receiver(null).shiftLeft(badDistance()); return 32; }
        catch (IllegalStateException e) { }
        if (trace != 13) return 33;
        active = BigInteger.valueOf(3);
        if (active.shiftLeft(replace()).intValueExact() != 6 || active.intValueExact() != 11) return 34;
        try { receiver(null).doubleValue(); return 27; }
        catch (NullPointerException e) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
