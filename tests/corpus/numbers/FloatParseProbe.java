interface DoubleReader { double read(String value); }

public class FloatParseProbe {
    static double parseDouble(String value) { return Double.parseDouble(value); }
    static float parseFloat(String value) { return Float.parseFloat(value); }
    static DoubleReader reader() { return Double::parseDouble; }
    static int run() {
        if (parseDouble(" 1.25 ") != 1.25 || parseFloat("\t1.25f ") != 1.25f) return 1;
        if (parseDouble("0x1.8p2") != 6.0 || parseFloat("0x1.8p2f") != 6.0f) return 2;
        if (Double.doubleToRawLongBits(parseDouble("-0.0")) != Long.MIN_VALUE) return 3;
        if (Float.floatToRawIntBits(parseFloat("-0x0p0")) != Integer.MIN_VALUE) return 4;
        if (reader().read(" 0x1.8p2 ") != 6.0) return 5;
        try { parseDouble("inf"); return 6; } catch (NumberFormatException expected) { }
        try { parseFloat("nan"); return 7; } catch (NumberFormatException expected) { }
        try { parseDouble(null); return 8; } catch (NullPointerException expected) { }
        try { parseFloat(null); return 9; } catch (NullPointerException expected) { }
        if (Double.doubleToRawLongBits(0x0.0000000000001p-1022) != 1L) return 10;
        if (Float.floatToRawIntBits(0x0.000002p-126f) != 1) return 11;
        if (Float.floatToRawIntBits(0x1.000001000000000000000001p0f) != 0x3f800001) return 12;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
