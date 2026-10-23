/** The primitive/String concat ABI must not depend on any builder or I/O facade. */
public class SourceOnlyConcatProbe {
    static boolean same(String left, String right) { return left == right; }
    static String combine(String value, int count, char last) { return value + count + last; }
    static String nullable(String value) { return "[" + value + "]"; }
    static int run() {
        String text = combine("é", 7, '\ud800');
        if (text.length() != 3 || text.charAt(0) != 'é' || text.charAt(1) != '7' || text.charAt(2) != '\ud800') return 1;
        if (!nullable(null).equals("[null]")) return 2;
        if (!same("a{" + '}', "a{}")) return 3;
        String source = "payload";
        if (same("" + source, source)) return 4;
        if (!("" + (1+2) + "x").equals("3x")) return 5;
        String unicodeSpace = "\u00a0 x \u00a0";
        if (!same(unicodeSpace.trim(), unicodeSpace)) return 6;
        String spaced = " \ud800 ";
        if (!spaced.trim().equals("\ud800")) return 7;
        if (!same(text.toString(), text)) return 8;
        return 0;
    }
}
