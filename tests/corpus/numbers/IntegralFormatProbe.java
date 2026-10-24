import static java.lang.Integer.toHexString;

interface IntegralHexReader { String read(char value); }

public class IntegralFormatProbe {
    static String boxed(Integer value) { return Integer.toHexString(value); }
    static String boxedLong(Long value) { return Long.toHexString(value); }
    static IntegralHexReader reader() { return Integer::toHexString; }
    static String decimal(Integer value) { return Integer.toString(value); }
    static int run() {
        if (!toHexString('A').equals("41") || !Long.toHexString('A').equals("41")) return 1;
        if (!Integer.toOctalString('A').equals("101") || !Long.toBinaryString('A').equals("1000001")) return 2;
        if (!Integer.toHexString((byte)-1).equals("ffffffff")) return 3;
        if (!Long.toHexString((short)-1).equals("ffffffffffffffff")) return 4;
        if (!Integer.toHexString(Integer.MIN_VALUE).equals("80000000")) return 5;
        if (!Long.toHexString(Long.MIN_VALUE).equals("8000000000000000")) return 6;
        if (!Integer.toOctalString(-1).equals("37777777777")) return 7;
        if (!Long.toOctalString(-1).equals("1777777777777777777777")) return 8;
        if (Integer.toBinaryString(-1).length() != 32 || Long.toBinaryString(-1).length() != 64) return 9;
        if (!Long.toHexString(0x100000000L).equals("100000000")) return 10;
        if (!boxed(65).equals("41") || !boxedLong(65L).equals("41")) return 11;
        try { boxed(null); return 12; } catch (NullPointerException expected) { }
        try { boxedLong(null); return 13; } catch (NullPointerException expected) { }
        if (!reader().read('A').equals("41")) return 14;
        if (!Integer.toString(Integer.MIN_VALUE).equals("-2147483648")) return 15;
        if (!Long.toString(Long.MIN_VALUE).equals("-9223372036854775808")) return 16;
        if (!Integer.toString(-1, 16).equals("-1") || !Long.toString(Long.MIN_VALUE, 16).equals("-8000000000000000")) return 17;
        if (!Integer.toString('A').equals("65") || !Long.toString((short)-1, 2).equals("-1")) return 18;
        if (!Integer.toString(35, 36).equals("z") || !Long.toString(10, 1).equals("10") || !Integer.toString(10, 37).equals("10")) return 19;
        if (!decimal(65).equals("65")) return 20;
        try { decimal(null); return 21; } catch (NullPointerException expected) { }
        if (Integer.toString(7) == Integer.toString(7)) return 22;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
