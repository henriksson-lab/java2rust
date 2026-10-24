public class StringReplaceProbe {
    public static int run() {
        String source = "1,234 / 5";
        String alias = source;
        String stripped = source.replace(",", "");
        if (!stripped.equals("1234 / 5") || !source.equals("1,234 / 5")) return 1;
        if (stripped == source || alias != source) return 2;
        if (source.replace("missing", "x") != source) return 3;
        String chars = "aba";
        if (chars.replace('a', 'a') != chars || chars.replace('z', 'q') != chars) return 4;
        CharSequence target = "a";
        CharSequence replacement = "b";
        if (!chars.replace(target, replacement).equals("bbb")) return 5;
        if (!"aaa".replace("aa", "x").equals("xa")) return 6;
        if (!"a$\\b".replace("$\\", "\\$").equals("a\\$b")) return 7;
        String raw = "a\ud800b\udc00c";
        if (!raw.replace("\ud800", "X").equals("aXb\udc00c")) return 8;
        if (!raw.replace('\ud800', '\ud801').equals("a\ud801b\udc00c")) return 9;
        if (!"\ud83d\ude00".replace("\ud83d", "X").equals("X\ude00")) return 10;
        if (!"\ud800A".replace("", "-").equals("-\ud800-A-")) return 11;
        if (!"".replace("", "-").equals("-")) return 12;
        if (!"ab".replace("a", "").equals("b")) return 13;
        String missing = null;
        try { source.replace(missing, ""); return 14; } catch (NullPointerException expected) { }
        try { source.replace("not present", missing); return 15; } catch (NullPointerException expected) { }
        try { missing.replace("a", "b"); return 16; } catch (NullPointerException expected) { }
        if (source.replace("no", "match") != alias || !stripped.equals("1234 / 5")) return 17;
        if (source.replace("", "") == source || !source.replace("", "").equals(source)) return 18;
        if (chars.replace("a", "a") != chars || chars.replace("ab", "ab") == chars) return 19;
        String empty = "";
        if (empty.replace("", "") == empty || !empty.replace("", "").isEmpty()) return 20;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
