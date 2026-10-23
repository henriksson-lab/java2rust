/** Java offsets count UTF-16 units, including when a search begins inside a pair. */
public class StringSearchProbe {
    static int locate(String text, String needle, int from) { return text.indexOf(needle, from); }
    static boolean same(String left, String right) { return left == right; }
    public static int run() {
        String text = new String("Aé😀Zé😀");
        if (!same(text, text)) return 1; // exercise the identity carrier too
        if (text.indexOf("Z") != 4 || text.lastIndexOf("é") != 5) return 2;
        if (locate(text, "😀", 3) != 6 || text.lastIndexOf("😀", 3) != 2) return 3;
        if (text.indexOf(0xd83d) != 2 || text.indexOf(0xde00) != 3) return 4;
        if (text.indexOf(0x1f600, 3) != 6 || text.lastIndexOf(0x1f600, 5) != 2) return 5;
        if (text.lastIndexOf(0xde00, 6) != 3 || text.lastIndexOf(0xd83d) != 6) return 6;
        if (text.indexOf("", 100) != 8 || text.lastIndexOf("", 100) != 8) return 7;
        if (text.indexOf("", -1) != 0 || text.lastIndexOf("", -1) != -1) return 8;
        if (text.indexOf("Z", 3) != 4 || text.lastIndexOf("Z", 3) != -1) return 9;
        if (text.indexOf(-1) != -1 || text.lastIndexOf(0x110000) != -1) return 10;
        if (!text.contains("😀Z") || !text.startsWith("Aé😀") || !text.endsWith("é😀")) return 11;
        if (text.contains("Z😀") || text.startsWith("é") || text.endsWith("Z")) return 12;
        if (text.indexOf('é', 2) != 5 || text.lastIndexOf('é', 4) != 1) return 13;
        if ("".indexOf("", 9) != 0 || "".lastIndexOf("", -1) != -1) return 14;
        if ("ascii/ascii".indexOf("ascii", 1) != 6 || "ascii/ascii".lastIndexOf("ascii", 5) != 0) return 15;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
