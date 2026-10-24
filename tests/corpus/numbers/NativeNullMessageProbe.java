import java.math.BigInteger;
import java.math.BigDecimal;
import java.math.RoundingMode;
import java.util.Objects;

public class NativeNullMessageProbe {
    static int events;
    static String missing() { events = events * 10 + 1; return null; }
    static String message(String value) { events = events * 10 + 2; return value; }
    static String require(String value, String message) { return Objects.requireNonNull(value, message); }
    static String replace(String value) { return value.replace("x", "y"); }
    static String replaceChar(String value) { return value.replace('x', 'y'); }
    static int sign(BigInteger value) { return value.signum(); }
    static BigDecimal divide(BigDecimal value, BigDecimal divisor, RoundingMode mode) { return value.divide(divisor, mode); }
    static BigDecimal divideScaled(BigDecimal value, BigDecimal divisor, int scale, RoundingMode mode) { return value.divide(divisor, scale, mode); }
    static BigDecimal scale(BigDecimal value, int scale, RoundingMode mode) { return value.setScale(scale, mode); }
    static boolean roundingMessage(NullPointerException error) { return error.getMessage().equals("Cannot read field \"oldMode\" because \"roundingMode\" is null"); }
    public static int run() {
        String text = "\ud800named\udc00";
        events = 0;
        try { require(missing(), message(text)); return 1; }
        catch (NullPointerException error) {
            if (events != 12 || error.getMessage() != text) return 2;
            if (error.getMessage().charAt(0) != '\ud800') return 3;
        }
        try { require(null, null); return 4; }
        catch (NullPointerException error) { if (error.getMessage() != null) return 5; }
        events = 0;
        String present = "present";
        if (require(present, message("ignored")) != present || events != 2) return 6;
        try { replace(null); return 7; }
        catch (NullPointerException error) {
            if (!error.getMessage().equals("Cannot invoke \"String.replace(java.lang.CharSequence, java.lang.CharSequence)\" because \"<parameter1>\" is null")) return 8;
        }
        try { replaceChar(null); return 9; }
        catch (NullPointerException error) {
            if (!error.getMessage().equals("Cannot invoke \"String.replace(char, char)\" because \"<parameter1>\" is null")) return 10;
        }
        try { sign(null); return 11; }
        catch (NullPointerException error) {
            if (!error.getMessage().equals("Cannot invoke \"java.math.BigInteger.signum()\" because \"<parameter1>\" is null")) return 12;
        }
        if (!replace("x").equals("y") || !replaceChar("x").equals("y")) return 13;
        if (sign(BigInteger.ONE) != 1) return 14;
        BigDecimal decimal = new BigDecimal(BigInteger.ONE);
        try { divide(decimal, decimal, null); return 15; }
        catch (NullPointerException error) { if (!roundingMessage(error)) return 16; }
        try { divideScaled(decimal, decimal, 2, null); return 17; }
        catch (NullPointerException error) { if (!roundingMessage(error)) return 18; }
        try { scale(decimal, 0, null); return 19; }
        catch (NullPointerException error) { if (!roundingMessage(error)) return 20; }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
