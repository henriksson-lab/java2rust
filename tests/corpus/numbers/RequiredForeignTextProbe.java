import java.math.BigInteger;
import java.math.BigDecimal;

public class RequiredForeignTextProbe {
    static String describe(BigInteger value) { return "n=" + value + "!"; }
    static void reject(BigInteger value) { throw new IllegalArgumentException("bad " + value); }
    public static int run() {
        BigInteger value = BigInteger.valueOf(123);
        if (!describe(value).equals("n=123!")) return 1;
        if (!("d=" + new BigDecimal("1.25")).equals("d=1.25")) return 2;
        if (!new StringBuilder().append(value).toString().equals("123")) return 3;
        if (value.intValue() != 123) return 4;
        try { reject(value); return 5; } catch (IllegalArgumentException error) {
            if (!error.getMessage().equals("bad 123")) return 6;
        }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
