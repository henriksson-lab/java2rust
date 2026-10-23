/** Source producer contracts; UTF-16 storage/indexing is tested separately. */
public class LiteralEscapesProbe {
    public static int run() {
        if ('\b' != 8 || '\f' != 12 || '\s' != 32) return 1;
        if ('\377' != 255 || '\u00e9' != 233 || '\uuuu0041' != 65) return 2;
        if ('\'' != 39 || '\\' != 92 || '\u005cn' != 10) return 3;
        if (!"\400\777\378".equals(" 0?7\0378")) return 4;
        if (!"\u00e9\uD83D\uDE00".equals("é😀")) return 5;
        if (!"\\u0041".equals("\\" + "u0041")) return 6;
        if (!"\\\u0041".equals("\\A")) return 7;
        if (!"\u005cn".equals("\n")) return 8;
        if (!"\u005c\u005cn".equals("\\n")) return 9;
        String block = """
            a\s
              b
            """;
        if (!block.equals("a \n  b\n")) return 10;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
