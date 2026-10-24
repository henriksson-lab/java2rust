import java.math.BigDecimal;
import java.math.BigInteger;
import java.math.RoundingMode;

/** Independent JDK requests for the runtime APIs used by BigFraction. */
public class BigDecimalDivisionOracle {
    interface Operation { BigDecimal run(); }
    static void observe(String request, Operation operation) {
        try {
            BigDecimal value = operation.run();
            System.out.println(request + "|" + value + ";" + value.scale() + ";" +
                Long.toHexString(Double.doubleToRawLongBits(value.doubleValue())) + ";" +
                Integer.toHexString(Float.floatToRawIntBits(value.floatValue())));
        } catch (Exception error) {
            System.out.println(request + "|ERR:" + error.getClass().getSimpleName() + ":" + error.getMessage());
        }
    }
    public static void main(String[] args) {
        String[] inputs = {"0", "0.00", "1", "1.00", "-1", "2", "3", "8", "-32",
            "0.125", "2.50", "-2.5", "1E+4", "1E-7",
            "123456789012345678901234567890", "100000000000000000000000000001"};
        for (String a : inputs) {
            for (String b : inputs) {
                BigDecimal left = new BigDecimal(a);
                BigDecimal right = new BigDecimal(b);
                observe("E|" + a + "|" + b, () -> left.divide(right));
                for (RoundingMode mode : RoundingMode.values()) {
                    observe("R|" + a + "|" + b + "|" + mode.ordinal(), () -> left.divide(right, mode));
                    for (int scale : new int[]{-2,0,1,4,12}) {
                        observe("T|" + a + "|" + b + "|" + scale + "|" + mode.ordinal(),
                            () -> left.divide(right, scale, mode));
                    }
                }
            }
        }
        for (String input : new String[]{"0","-1","123456789012345678901234567890",
                "1606938044258990275541962092341162602522202993782792835301376"}) {
            observe("I|" + input, () -> new BigDecimal(new BigInteger(input)));
        }
    }
}
