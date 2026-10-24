import static java.lang.Integer.toUnsignedLong;
import static java.lang.Long.compareUnsigned;
import static java.lang.Long.signum;
import static java.lang.Math.copySign;

public class NativeNumericProbe {
    static int order;
    static int operand(int value) { order = order * 10 + value; return value; }
    static int unsignedFail(int value) { return Integer.divideUnsigned(value, 0); }
    static long exactFail(long value) { return Math.multiplyExact(value, 2); }

    static int run() {
        if (Double.doubleToRawLongBits(copySign(0.0, -1.0)) != Long.MIN_VALUE) return 1;
        if (Float.floatToRawIntBits(Math.copySign(0.0f, -1.0f)) != Integer.MIN_VALUE) return 2;
        if (Math.copySign(-2.0f, 1.0f) != 2.0f || Math.copySign(-2.0, 1.0) != 2.0) return 3;
        if (Math.getExponent(0.0) != -1023 || Math.getExponent(Double.MIN_VALUE) != -1023) return 4;
        if (Math.getExponent(Double.MIN_NORMAL) != -1022 || Math.getExponent(Double.MAX_VALUE) != 1023) return 5;
        if (Math.getExponent(Double.NaN) != 1024 || Math.getExponent(Double.NEGATIVE_INFINITY) != 1024) return 6;
        if (Math.getExponent(0.0f) != -127 || Math.getExponent(Float.MIN_VALUE) != -127) return 7;
        if (Math.getExponent(Float.MIN_NORMAL) != -126 || Math.getExponent(Float.MAX_VALUE) != 127) return 8;
        if (Math.getExponent(Float.NaN) != 128 || Math.getExponent(Float.POSITIVE_INFINITY) != 128) return 9;
        if (Math.getExponent('a') != 6 || Math.copySign('a', -1) != -97.0f) return 10;
        if (toUnsignedLong(-1) != 4294967295L || Integer.toUnsignedLong(Integer.MIN_VALUE) != 2147483648L) return 11;
        if (Math.multiplyExact(operand(2), operand(3)) != 6 || order != 23) return 12;
        if (Math.multiplyExact(3L, -4) != -12L || Math.multiplyExact(3L, -4L) != -12L) return 13;
        try { Math.multiplyExact(Integer.MAX_VALUE, 2); return 14; }
        catch (ArithmeticException error) { if (!error.getMessage().equals("integer overflow")) return 15; }
        try { exactFail(Long.MAX_VALUE); return 16; }
        catch (ArithmeticException error) { if (!error.getMessage().equals("long overflow")) return 17; }
        if (Integer.divideUnsigned(-1, 3) != 1431655765 || Integer.remainderUnsigned(-1, 3) != 0) return 18;
        if (Long.divideUnsigned(-1L, 2L) != Long.MAX_VALUE || Long.remainderUnsigned(-1L, 2L) != 1) return 19;
        if (Integer.divideUnsigned(Integer.MIN_VALUE, -1) != 0 || Integer.remainderUnsigned(Integer.MIN_VALUE, -1) != Integer.MIN_VALUE) return 20;
        try { unsignedFail(1); return 21; }
        catch (ArithmeticException error) { if (!error.getMessage().equals("/ by zero")) return 22; }
        try { Long.remainderUnsigned(1L, 0L); return 23; }
        catch (ArithmeticException error) { if (!error.getMessage().equals("/ by zero")) return 24; }
        if (Integer.compare(Integer.MIN_VALUE, Integer.MAX_VALUE) != -1 ||
                Integer.compare(Integer.MAX_VALUE, Integer.MIN_VALUE) != 1 || Integer.compare(-3, -3) != 0) return 25;
        if (Long.compare(Long.MIN_VALUE, Long.MAX_VALUE) != -1 ||
                Long.compare(Long.MAX_VALUE, Long.MIN_VALUE) != 1 || Long.compare(-3L, -3L) != 0) return 26;
        if (Integer.compareUnsigned(-1, 0) != 1 || Integer.compareUnsigned(0, -1) != -1 ||
                Integer.compareUnsigned(Integer.MIN_VALUE, -1) != -1 || Integer.compareUnsigned(-1, -1) != 0) return 27;
        if (compareUnsigned(-1L, 0L) != 1 || Long.compareUnsigned(0L, -1L) != -1 ||
                Long.compareUnsigned(Long.MIN_VALUE, -1L) != -1 || Long.compareUnsigned(-1L, -1L) != 0) return 28;
        // Widen to the declared signed parameter type before reinterpreting as unsigned.
        if (Long.compareUnsigned(-1, 4294967295L) != 1 || Long.compare('a', 97L) != 0) return 29;
        order = 0;
        if (Integer.compareUnsigned(operand(2), operand(3)) != -1 || order != 23) return 30;
        Integer boxed = 7;
        if (Long.compare(boxed, 6L) != 1 || Integer.compareUnsigned(boxed, 8) != -1) return 31;
        boxed = null;
        try { Long.compareUnsigned(boxed, 0L); return 32; }
        catch (NullPointerException error) { }
        if (Integer.signum(Integer.MIN_VALUE) != -1 || Integer.signum(0) != 0 ||
                Integer.signum(Integer.MAX_VALUE) != 1) return 33;
        if (signum(Long.MIN_VALUE) != -1 || signum(0L) != 0 || signum(Long.MAX_VALUE) != 1) return 34;
        if (Long.signum(-2) != -1 || Integer.signum('a') != 1) return 35;
        boxed = -7;
        order = 0;
        if (Integer.signum(operand(2)) != 1 || order != 2 || Long.signum(boxed) != -1) return 36;
        boxed = null;
        try { Integer.signum(boxed); return 37; }
        catch (NullPointerException error) { }
        try { Long.signum(boxed); return 38; }
        catch (NullPointerException error) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
