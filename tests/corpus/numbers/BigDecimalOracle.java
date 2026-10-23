import java.math.BigDecimal;
import java.math.RoundingMode;

/** Independent request/observation stream for the explicitly modeled JDK API. */
public final class BigDecimalOracle {
    private static String view(BigDecimal value) {
        return value.toString() + ";" + value.scale() + ";" +
            Long.toHexString(Double.doubleToRawLongBits(value.doubleValue())) + ";" +
            Integer.toHexString(Float.floatToRawIntBits(value.floatValue()));
    }
    private interface Operation { BigDecimal run(); }
    private static void observe(String request, Operation operation) {
        try { System.out.println(request + "|" + view(operation.run())); }
        catch (Exception error) {
            System.out.println(request + "|ERR:" + error.getClass().getSimpleName() + ":" + error.getMessage());
        }
    }
    public static void main(String[] args) {
        String[] inputs = {"0", "-0", "0.00", "1", "-1", "1.00", "2.5", "3.5",
            "-2.5", "-3.5", "1.25", "-1.25", "1.00000001", "999.995", "-999.995",
            ".001", "1E+20", "1E-20", "123456789012345678901234567890.12345",
            "١٢.٣", "0E+2147483647", "0E-2147483647"};
        int[] scales = {-10, -3, -1, 0, 1, 2, 6, 15, 40};
        for (String input : inputs) {
            observe("S|" + input, () -> new BigDecimal(input));
            for (int scale : scales) {
                for (RoundingMode mode : RoundingMode.values()) {
                    observe("R|" + input + "|" + scale + "|" + mode.ordinal(),
                        () -> new BigDecimal(input).setScale(scale, mode));
                }
            }
            for (String other : inputs) {
                observe("A|" + input + "|" + other, () -> new BigDecimal(input).add(new BigDecimal(other)));
                observe("B|" + input + "|" + other, () -> new BigDecimal(input).subtract(new BigDecimal(other)));
            }
        }
        long[] bits = {0L, Long.MIN_VALUE, 0x3fb999999999999aL, 0x3fc999999999999aL,
            0x3ff0000000000000L, 0x4004000000000000L, 1L, 0x000fffffffffffffL,
            0x0010000000000000L, 0x7fefffffffffffffL, 0x7ff0000000000000L,
            0xfff0000000000000L, 0x7ff8000000000000L, 0xfff8000000000000L,
            0x3fefffffffffffffL, 0x3ff0000000000001L, 0x8000000000000001L};
        for (long left : bits) {
            observe("D|" + Long.toHexString(left), () -> new BigDecimal(Double.longBitsToDouble(left)));
            for (long right : bits) {
                observe("Q|" + Long.toHexString(left) + "|" + Long.toHexString(right),
                    () -> new BigDecimal(Double.longBitsToDouble(left)).add(new BigDecimal(Double.longBitsToDouble(right))));
            }
        }
        for (String invalid : new String[]{"", "+", ".", "1e", "1e+", "1.0e-2147483647", "1..2", "x", " 1", "1e99999999999", "1e2x", "1e--2", "𝟙"}) {
            observe("S|" + invalid, () -> new BigDecimal(invalid));
        }
        for (String input : new String[]{"1E+2147483647", "1E-2147483647", "0E+2147483647"}) {
            for (int scale : new int[]{-1, 0, 1, Integer.MIN_VALUE, Integer.MAX_VALUE}) {
                observe("R|" + input + "|" + scale + "|1", () -> new BigDecimal(input).setScale(scale, RoundingMode.DOWN));
            }
        }
    }
}
