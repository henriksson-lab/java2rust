package signmagnitude;
import java.math.BigInteger;
public class SignMagnitudeProbe {
    static byte[] retained;
    static int calls;
    static int sign() { calls++; return 2; }
    static byte[] missing() { calls++; return null; }
    static BigInteger make(int sign, byte[] magnitude) { return new BigInteger(sign, magnitude); }
    public static int run() {
        byte[] bytes = {0, -128, 1};
        retained = bytes;
        BigInteger positive = make(1, bytes);
        BigInteger negative = make(-1, retained);
        bytes[1] = 0;
        if (!positive.toString().equals("32769") || !negative.toString().equals("-32769")) return 1;
        if (!make(0, new byte[0]).toString().equals("0")) return 2;
        if (!make(-1, new byte[] {0, 0}).toString().equals("0")) return 3;
        BigInteger wide = make(1, new byte[] {-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1});
        if (!wide.toString().equals("340282366920938463463374607431768211455")) return 4;
        try { make(2, new byte[0]); return 5; }
        catch (NumberFormatException error) { if (!"Invalid signum value".equals(error.getMessage())) return 6; }
        try { make(0, new byte[] {0, 1}); return 7; }
        catch (NumberFormatException error) { if (!"signum-magnitude mismatch".equals(error.getMessage())) return 8; }
        try { make(sign(), missing()); return 9; }
        catch (NullPointerException expected) { if (calls != 2) return 10; }
        if (!new BigInteger("32769").toString().equals(positive.toString())) return 11;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
