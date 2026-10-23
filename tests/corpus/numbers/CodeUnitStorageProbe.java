/** Java char is an unsigned 16-bit unit, not a Unicode scalar. */
import java.util.Arrays;
public class CodeUnitStorageProbe {
    static char defaultUnit;
    static char relay(char unit) { return unit; }
    static int select(char unit) {
        switch (unit) { case '\ud83d': return 1; case '\ude00': return 2; default: return 0; }
    }
    public static int run() {
        char high = '\ud83d';
        char low = (char)0xde00;
        if ((int)high != 0xd83d || relay(low) != low) return 1;
        if (!(high < low) || high + 1 != 0xd83e || low - high != 1475) return 2;
        char[] units = {high, low, (char)-1};
        char[] zero = new char[2];
        if (units[2] != 65535 || zero[0] != 0 || defaultUnit != 0) return 3;
        char updated = (char)65535;
        if (updated++ != 65535 || updated != 0 || --updated != 65535) return 4;
        updated += 2;
        if (updated != 1 || select(high) != 1 || select(low) != 2) return 5;
        if ((int)(char)-1.75 != 65535 || (int)(char)Double.NaN != 0) return 6;
        if ((int)(char)Double.POSITIVE_INFINITY != 65535 || (int)(char)Double.NEGATIVE_INFINITY != 0) return 7;
        if ((int)(char)0x10000ffffL != 65535 || (char)high != high) return 8;
        Character boxed = high;
        if ((int)boxed != 0xd83d || boxed.charValue() != high) return 9;
        Object erased = boxed;
        if ((int)(Character)erased != 0xd83d) return 13;
        if (!Arrays.equals(units, new char[]{high, low, (char)-1}) || Arrays.hashCode(units) != 55055195) return 14;
        if (Arrays.compare(new char[]{high}, new char[]{low}) != -1475) return 15;
        String text = "Aé😀Z";
        if (text.charAt(2) != high || text.charAt(3) != low || text.charAt(4) != 'Z') return 10;
        try { text.charAt(-1); return 11; } catch (StringIndexOutOfBoundsException ex) { }
        try { text.charAt(5); return 12; } catch (StringIndexOutOfBoundsException ex) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
