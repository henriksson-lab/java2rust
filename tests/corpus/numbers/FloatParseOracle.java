import java.nio.charset.StandardCharsets;

public class FloatParseOracle {
    static void sample(String value) {
        StringBuilder bytes = new StringBuilder();
        for (byte b : value.getBytes(StandardCharsets.UTF_8)) bytes.append(String.format("%02x", b & 255));
        try { System.out.println("D|" + bytes + "|" + Long.toHexString(Double.doubleToRawLongBits(FloatParseProbe.parseDouble(value)))); }
        catch (Exception e) { System.out.println("D|" + bytes + "|ERR:" + e.getClass().getSimpleName() + ":" + e.getMessage()); }
        try { System.out.println("F|" + bytes + "|" + Integer.toHexString(Float.floatToRawIntBits(FloatParseProbe.parseFloat(value)))); }
        catch (Exception e) { System.out.println("F|" + bytes + "|ERR:" + e.getClass().getSimpleName() + ":" + e.getMessage()); }
    }
    public static void main(String[] args) {
        String[] inputs = {"", " ", "\t", "1", "+1", "-1", "0.0", "-0.0", ".5", "1.", "1.e2", ".5e-2",
            " 1.25 ", "\t-1.25f ", "\u00001.25\u0020", "1d", "1F", "1D", "1.25e+10f", "1.234567890123456789",
            "NaN", "+NaN", "-NaN", "Infinity", "+Infinity", "-Infinity", " NaN ", "inf", "nan", "NAN", "infinity",
            "NaNf", "Infinityd", "1_000", "１２", "é", "😀", "1 2", "1.2.3", ".", "+", "--1", "1e", "1e+", "1ee2", "1ff",
            "0x1p0", "-0X1.8P+2", "+0x.8p-2", "0x1.p3", "0x0p999999999999999999999999", "-0x0p-999999999999999999999999",
            "0x1p1023", "0x1p1024", "-0x1p-1075", "0x1.8p-1075", "0x1.fffffffffffffp1023", "0x1.fffffffffffff8p1023",
            "0x0.0000000000001p-1022", "0x0.000002p-126f", "0x1.000001000000000000000001p0f", "0x1.000001p0f",
            "0x1p0d", "0x1p0f", "0x1p999999999999999999999999", "0x1p-999999999999999999999999",
            "0x", "0xp0", "0x.p0", "0x1", "0x1p", "0x1p+", "0x1p0p0", "0x1.2.3p0", "0xg.p0", "0x1p1.0",
            "2.2250738585072014e-308", "4.9406564584124654e-324", "1.7976931348623157e308", "1e309", "-1e-9999",
            "3.4028234663852886e38", "1.401298464324817e-45", "1.000000059604644775390625000001"};
        for (String value : inputs) sample(value);
        // Typical decimal/configuration values plus Java's own hex round trips.
        for (int i=-100; i<=100; i++) {
            sample(Double.toString(i / 10.0)); sample(Double.toHexString(i / 10.0));
        }
    }
}
