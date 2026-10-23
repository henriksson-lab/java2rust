public class ScalbOracle {
    static final int[] EXPONENTS = {
        Integer.MIN_VALUE, -2100, -2099, -2098, -1536, -1075, -1074, -1024, -1023,
        -1022, -1021, -1020, -513, -512, -511, -279, -278, -277, -150, -149, -148,
        -128, -127, -126, -125, -25, -24, -23, -2, -1, 0, 1, 2, 23, 24, 25,
        125, 126, 127, 128, 148, 149, 150, 277, 278, 279, 511, 512, 513,
        1020, 1021, 1022, 1023, 1024, 1074, 1075, 1536, 2098, 2099, 2100, Integer.MAX_VALUE
    };
    static void d(long bits, int exponent) {
        double value = Double.longBitsToDouble(bits);
        System.out.println("D|" + Long.toHexString(bits) + "|" + exponent + "|"
            + Long.toHexString(Double.doubleToRawLongBits(ScalbProbe.scaleDouble(value, exponent))) + "|"
            + Long.toHexString(Double.doubleToRawLongBits(ScalbProbe.strictDouble(value, exponent))));
    }
    static void f(int bits, int exponent) {
        float value = Float.intBitsToFloat(bits);
        System.out.println("F|" + Integer.toHexString(bits) + "|" + exponent + "|"
            + Integer.toHexString(Float.floatToRawIntBits(ScalbProbe.scaleFloat(value, exponent))) + "|"
            + Integer.toHexString(Float.floatToRawIntBits(ScalbProbe.strictFloat(value, exponent))));
    }
    public static void main(String[] args) {
        long[] doubles = {0, 1, 2, 3, 4, 5, 7, 0xfffffffffffffL, 0x10000000000000L,
            0x10000000000001L, 0x10000000000002L, 0x10000000000003L,
            0x3fdfffffffffffffL, 0x3fe0000000000000L, 0x3fe0000000000001L,
            0x3fefffffffffffffL, 0x3ff0000000000000L, 0x3ff0000000000001L,
            0x3ff0000000000003L, 0x7feffffffffffffeL, 0x7fefffffffffffffL,
            0x7ff0000000000000L, 0x7ff8000000000000L, 0x7ff8000000001234L};
        int[] floats = {0, 1, 2, 3, 4, 5, 7, 0x7fffff, 0x800000, 0x800001,
            0x800002, 0x800003, 0x3effffff, 0x3f000000, 0x3f000001, 0x3f7fffff,
            0x3f800000, 0x3f800001, 0x3f800003, 0x7f7ffffe, 0x7f7fffff,
            0x7f800000, 0x7fc00000, 0x7fc01234};
        for (long bits : doubles) for (int exponent : EXPONENTS) {
            d(bits, exponent); d(bits | Long.MIN_VALUE, exponent);
        }
        for (int bits : floats) for (int exponent : EXPONENTS) {
            f(bits, exponent); f(bits | Integer.MIN_VALUE, exponent);
        }
        long state = 0x123456789abcdefL;
        for (int i = 0; i < 5000; i++) {
            state = state * 6364136223846793005L + 1442695040888963407L;
            // Broad finite samples avoid unstable
            // signaling-NaN transport through the test driver's conversions.
            long bits = state & 0xffefffffffffffffL;
            d(bits, (int) ((state >>> 32) % 4401) - 2200);
            int raw = (int) state & 0xff7fffff;
            f(raw, (int) ((state >>> 32) % 601) - 300);
        }
    }
}
