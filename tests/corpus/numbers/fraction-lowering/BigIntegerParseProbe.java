package integerparse;

import java.math.BigInteger;

public class BigIntegerParseProbe {
    static BigInteger parse(String value) { return new BigInteger(value); }
    public static int run() {
        String source = "+000123456789012345678901234567890";
        BigInteger value = parse(source);
        if (!value.toString().equals("123456789012345678901234567890")) return 1;
        if (!source.equals("+000123456789012345678901234567890")) return 2;
        if (!parse("-0000").toString().equals("0")) return 3;
        if (!parse("١٢٣").toString().equals("123") || !parse("-１２３").toString().equals("-123")) return 4;
        if (!parse("1606938044258990275541962092341162602522202993782792835301376").toString()
                .equals(BigInteger.valueOf(2).pow(200).toString())) return 5;
        try { parse(""); return 6; }
        catch (NumberFormatException error) {
            if (!"Zero length BigInteger".equals(error.getMessage())) return 7;
        }
        try { parse("+"); return 8; }
        catch (NumberFormatException error) {
            if (!"Zero length BigInteger".equals(error.getMessage())) return 9;
        }
        try { parse("1-2"); return 10; }
        catch (NumberFormatException error) {
            if (!"Illegal embedded sign character".equals(error.getMessage())) return 11;
        }
        try { parse("123456789x"); return 12; }
        catch (NumberFormatException error) {
            if (!"For input string: \"23456789x\"".equals(error.getMessage())) return 13;
        }
        try { parse("\ud800"); return 14; }
        catch (NumberFormatException error) {
            if (!"For input string: \"\ud800\"".equals(error.getMessage())) return 15;
        }
        try { parse("𝟙"); return 16; }
        catch (NumberFormatException error) {
            if (!"For input string: \"𝟙\"".equals(error.getMessage())) return 17;
        }
        try { parse(null); return 18; }
        catch (NullPointerException expected) { }
        if (!value.toString().equals("123456789012345678901234567890")) return 19;
        return 0;
    }
    public static void main(String[] args) {
        int result = run();
        if (result != 0) throw new AssertionError("integer parse contract: " + result);
        System.out.println(result);
    }
}
