class CharArrayHolder {
    char[] units;
    CharArrayHolder(char[] units) { this.units = units; }
}
class ArraySource {
    char[] held;
    ArraySource(char[] value) { held = value; }
    char[] toCharArray() { return held; }
}

public class FreshNativeArrayContracts {
    static char[] retained;
    static int effects;
    static String source() { effects++; return "\ud800x"; }
    static char[] keep(char[] value) { retained = value; return value; }
    static char[] create() { return source().toCharArray(); }
    static char[] from(String value) { return value.toCharArray(); }
    static int inspect(char[] value) { return value[0]; }
    static int run() {
        char[] first = keep(create());
        char[] second = keep(source().toCharArray());
        if (effects != 2 || first == second) return 1;
        first[0] = 'q';
        if (second[0] != '\ud800' || retained != second) return 2;
        CharArrayHolder holder = new CharArrayHolder(source().toCharArray());
        if (effects != 3 || holder.units[0] != '\ud800') return 3;
        char[] alias = holder.units;
        alias[1] = 'z';
        if (holder.units[1] != 'z') return 4;
        char[] explicit = keep(second);
        if (explicit != second) return 5;
        explicit[1] = 'y';
        if (retained[1] != 'y') return 6;
        if (from("\udc00")[0] != '\udc00') return 7;
        try { from(null); return 8; }
        catch (NullPointerException expected) { }
        String reusable = source();
        char[] third = keep(reusable.toCharArray());
        if (!reusable.equals("\ud800x") || third[0] != '\ud800' || effects != 4) return 9;
        ArraySource other = new ArraySource(second);
        if (keep(other.toCharArray()) != second) return 10;
        if (inspect(retained) != 0xd800 || inspect(source().toCharArray()) != 0xd800 || effects != 5) return 11;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
