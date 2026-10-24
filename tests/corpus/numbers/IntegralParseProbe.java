import static java.lang.Integer.parseInt;

public class IntegralParseProbe {
    public static int run() {
        if (parseInt("+00042") != 42 || Integer.parseInt("１２３") != 123 || Integer.parseInt("-١٢٣") != -123) return 1;
        if (Integer.parseInt("-2147483648") != Integer.MIN_VALUE || Integer.parseInt("2147483647") != Integer.MAX_VALUE) return 2;
        if (Long.parseLong("-9223372036854775808") != Long.MIN_VALUE || Long.parseLong("9223372036854775807") != Long.MAX_VALUE) return 3;
        if (Byte.parseByte("-128") != -128 || Short.parseShort("32767") != 32767) return 4;
        if (Integer.parseInt("-80000000", 16) != Integer.MIN_VALUE || Long.parseLong("-8000000000000000", 16) != Long.MIN_VALUE) return 5;
        if (Integer.parseInt("Ｚ", 36) != 35 || Integer.parseInt("101", 2) != 5 || Short.parseShort("177", 8) != 127) return 6;
        if (Integer.parseInt("-0") != 0 || Integer.parseInt(" +12 ".trim()) != 12) return 7;
        try { Integer.parseInt("2147483648"); return 8; }
        catch (NumberFormatException e) { if (!e.getMessage().equals("For input string: \"2147483648\"")) return 9; }
        try { Long.parseLong("9223372036854775808"); return 10; }
        catch (NumberFormatException e) { if (!e.getMessage().equals("For input string: \"9223372036854775808\"")) return 11; }
        try { Integer.parseInt(" 1"); return 12; }
        catch (NumberFormatException e) { if (!e.getMessage().equals("For input string: \" 1\"")) return 13; }
        try { Integer.parseInt("g", 16); return 14; }
        catch (NumberFormatException e) { if (!e.getMessage().equals("For input string: \"g\" under radix 16")) return 15; }
        try { Byte.parseByte("128"); return 16; }
        catch (NumberFormatException e) { if (!e.getMessage().equals("Value out of range. Value:\"128\" Radix:10")) return 17; }
        try { Short.parseShort("-32769"); return 18; }
        catch (NumberFormatException e) { if (!e.getMessage().equals("Value out of range. Value:\"-32769\" Radix:10")) return 19; }
        String missing = null;
        try { Integer.parseInt(missing, 1); return 20; }
        catch (NumberFormatException e) { if (!e.getMessage().equals("Cannot parse null string")) return 21; }
        try { Integer.parseInt("1", 1); return 22; }
        catch (NumberFormatException e) { if (!e.getMessage().equals("radix 1 less than Character.MIN_RADIX")) return 23; }
        try { Long.parseLong("1", 37); return 24; }
        catch (NumberFormatException e) { if (!e.getMessage().equals("radix 37 greater than Character.MAX_RADIX")) return 25; }
        try { Integer.parseInt("\ud800"); return 26; }
        catch (NumberFormatException e) { if (!e.getMessage().equals("For input string: \"\ud800\"")) return 27; }
        try { Integer.parseInt(""); return 28; }
        catch (NumberFormatException e) { if (!e.getMessage().equals("For input string: \"\"")) return 29; }
        if (Integer.parseInt("+101", 2) != 5 || Byte.parseByte("7f", 16) != 127) return 30;
        try { Integer.parseInt((String)null); return 31; }
        catch (NumberFormatException e) { if (!e.getMessage().equals("Cannot parse null string")) return 32; }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
