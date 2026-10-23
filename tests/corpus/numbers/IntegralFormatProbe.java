import static java.lang.Integer.toHexString;

interface IntegralHexReader { String read(char value); }

public class IntegralFormatProbe {
    static String boxed(Integer value) { return Integer.toHexString(value); }
    static String boxedLong(Long value) { return Long.toHexString(value); }
    static IntegralHexReader reader() { return Integer::toHexString; }
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
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
