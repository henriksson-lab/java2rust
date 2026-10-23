import static java.lang.Math.scalb;

public class ScalbProbe {
    static int order;
    static double operand() { order = order * 10 + 1; return 1.5; }
    static int exponent() { order = order * 10 + 2; return 2; }
    static double scaleDouble(double value, int exponent) { return Math.scalb(value, exponent); }
    static float scaleFloat(float value, int exponent) { return Math.scalb(value, exponent); }
    static double strictDouble(double value, int exponent) { return StrictMath.scalb(value, exponent); }
    static float strictFloat(float value, int exponent) { return java.lang.StrictMath.scalb(value, exponent); }
    static double unboxDouble(Double value) { return Math.scalb(value, 1); }
    static float unboxFloat(Float value) { return Math.scalb(value, 1); }
    static int run() {
        if (scalb(operand(), exponent()) != 6.0 || order != 12) return 1;
        if (Float.floatToRawIntBits(Math.scalb(16777217, 0)) != 0x4b800000) return 2;
        if (Math.scalb('a', 1) != 194.0f) return 3;
        if (Math.scalb(3L, -1) != 1.5f) return 4;
        if (Math.scalb((byte) 3, 1) != 6.0f || Math.scalb((short) 3, 2) != 12.0f) return 5;
        Double boxedDouble = 1.5;
        Float boxedFloat = 1.5f;
        if (Math.scalb(boxedDouble, 1) != 3.0 || Math.scalb(boxedFloat, 2) != 6.0f) return 6;
        if (strictDouble(-0.0, Integer.MAX_VALUE) != -0.0) return 7;
        try { unboxDouble(null); return 8; } catch (NullPointerException expected) { }
        try { unboxFloat(null); return 9; } catch (NullPointerException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
