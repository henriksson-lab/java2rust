import java.util.Arrays;

public class CsvTextContracts {
    static final char RS = 30;
    static final char HIGH = 0xd800;
    static final char LAST = 65535;
    static String[] held;
    static char[] keptChars;
    static int evaluations;
    static String[] retain(String[] values) { held = values; return values; }
    static char[] retainChars(char[] values) { keptChars = values; return values; }
    static char[] argument() { evaluations++; return null; }
    static int index() { evaluations++; return 0; }
    static String character(char unit) { return String.valueOf(unit); }
    static char[] charsResult() { return keptChars; }
    static String names(String[] values) { return Arrays.toString(values); }
    static String chars(char[] values) { return Arrays.toString(values); }
    static int run() {
        if (!Character.isWhitespace(RS)) return 1;
        if (Character.isWhitespace('\u0085') || Character.isWhitespace('\u00a0')) return 2;
        if (!Character.isWhitespace('\u2003')) return 3;
        if (Character.isWhitespace('\u2007') || Character.isWhitespace('\u202f')) return 4;
        if (Character.isWhitespace(HIGH) || Character.isWhitespace(LAST)) return 5;
        if (Character.isWhitespace(-1) || Character.isWhitespace(0x110000)) return 6;
        if (!Character.isWhitespace(0x2028)) return 7;
        if (!character(HIGH).equals("\ud800")) return 8;
        if (!character('x').equals("x")) return 9;
        if (!names(new String[]{"\ud800", null, "\udc00"}).equals("[\ud800, null, \udc00]")) return 10;
        if (!chars(new char[]{HIGH, 'x', '\udc00'}).equals("[\ud800, x, \udc00]")) return 11;
        if (!names(null).equals("null")) return 12;
        if (!names(new String[0]).equals("[]")) return 13;
        String[] retained = retain(new String[]{"\ud800", null});
        String[] equal = new String[]{"\ud800", null};
        if (!Arrays.equals(retained, retained)) return 14;
        if (!Arrays.equals(retained, equal)) return 15;
        if (Arrays.hashCode(retained) != 31 * (31 + 0xd800)) return 16;
        if (!names(held).equals("[\ud800, null]")) return 17;
        held[0] = "\ud801";
        if (Arrays.equals(retained, equal)) return 18;
        if (Arrays.hashCode(retained) == Arrays.hashCode(equal)) return 19;
        if (Arrays.hashCode((String[])null) != 0) return 20;
        if (!Arrays.equals((String[])null, (String[])null)) return 21;
        char[] units = retainChars(new char[]{HIGH, 'x', '\udc00'});
        StringBuilder builder = new StringBuilder();
        builder.append(units);
        if (!builder.toString().equals("\ud800x\udc00")) return 22;
        if (!String.valueOf(units).equals("\ud800x\udc00")) return 23;
        keptChars[1] = 'y';
        builder.append(units);
        if (!builder.toString().equals("\ud800x\udc00\ud800y\udc00")) return 24;
        try { builder.append((char[])null); return 25; }
        catch (NullPointerException expected) { }
        try { String.valueOf((char[])null); return 26; }
        catch (NullPointerException expected) { }
        StringBuilder absent = null;
        try { absent.append(argument()); return 27; }
        catch (NullPointerException expected) { }
        if (evaluations != 1) return 28;
        if (!Arrays.equals(retained, 0, 1, retained, 0, 1)) return 29;
        try { Arrays.equals(retained, 2, 1, (String[])null, 0, 0); return 30; }
        catch (IllegalArgumentException expected) { }
        try { Arrays.equals((String[])null, 2, 1, retained, 0, 0); return 31; }
        catch (NullPointerException expected) { }
        try { Arrays.equals(retained, 0, 3, retained, 0, 1); return 32; }
        catch (ArrayIndexOutOfBoundsException expected) { }
        try { Arrays.equals((String[])null, index(), 0, retained, 0, 0); return 33; }
        catch (NullPointerException expected) { }
        if (evaluations != 2) return 34;
        String snapshot = new String(charsResult());
        if (!snapshot.equals("\ud800y\udc00")) return 35;
        keptChars[1] = 'z';
        if (!snapshot.equals("\ud800y\udc00")) return 36;
        if (!new String(charsResult()).equals("\ud800z\udc00")) return 37;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
