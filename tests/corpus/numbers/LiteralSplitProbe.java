/** Literal regex specialization must not change logical text or split limits. */
public class LiteralSplitProbe {
    static final String SEPARATOR = " ";
    static boolean same(String left, String right) { return left == right; }
    static String[] split(String value, int limit) { return value.split(SEPARATOR, limit); }
    public static int run() {
        String value = "\ud800 a  \udc00 ";
        String[] parts = split(value, 0);
        if (parts.length != 4 || !parts[0].equals("\ud800") || !parts[1].equals("a")
                || !parts[2].equals("") || !parts[3].equals("\udc00")) return 1;
        String[] bounded = split(value, 2);
        if (bounded.length != 2 || !bounded[0].equals("\ud800")
                || !bounded[1].equals("a  \udc00 ")) return 2;
        String[] keep = split(value, -1);
        if (keep.length != 5 || !keep[4].equals("")) return 3;
        if (split("  ", 0).length != 0 || split("  ", -1).length != 3) return 4;
        String empty = new String("");
        String[] emptyParts = split(empty, 0);
        if (emptyParts.length != 1 || !same(emptyParts[0], empty)) return 5;
        String noMatch = new String("\ud800abc\udc00");
        String[] whole = split(noMatch, 0);
        if (whole.length != 1 || !same(whole[0], noMatch)) return 6;
        String[] once = split(value, 1);
        if (once.length != 1 || !same(once[0], value)) return 7;
        String[] multi = "::a::::b::".split("::", -1);
        if (multi.length != 5 || !multi[0].equals("") || !multi[1].equals("a")
                || !multi[2].equals("") || !multi[3].equals("b") || !multi[4].equals("")) return 8;
        String[] unicode = "\ud83d\ude00é\ud800é".split("é");
        if (unicode.length != 2 || !unicode[0].equals("\ud83d\ude00")
                || !unicode[1].equals("\ud800")) return 9;
        if (split(value, Integer.MAX_VALUE).length != 5) return 10;
        if (split(value, Integer.MIN_VALUE).length != 5) return 11;
        String absent = null;
        try { absent.split(SEPARATOR); return 12; }
        catch (NullPointerException expected) { }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
