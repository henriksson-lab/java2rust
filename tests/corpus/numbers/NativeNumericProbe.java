import static java.lang.Integer.toUnsignedLong;
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
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
