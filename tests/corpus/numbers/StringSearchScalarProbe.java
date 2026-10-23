/** Same search boundary without source reference-identity representation. */
public class StringSearchScalarProbe {
    static int locate(String value, String needle, int from) { return value.indexOf(needle, from); }
    public static int run() {
        String value = "Aé😀Z";
        if (locate(value, "Z", 3) != 4 || value.lastIndexOf("é", 2) != 1) return 1;
        if (value.indexOf(0xd83d) != 2 || value.lastIndexOf(0xde00) != 3) return 2;
        if (value.indexOf(0x1f600, 3) != -1 || value.lastIndexOf(0x1f600, 3) != 2) return 3;
        if (value.indexOf("", 9) != 5 || value.lastIndexOf("", -1) != -1) return 4;
        if (!value.contains("é😀") || !value.startsWith("Aé") || !value.endsWith("😀Z")) return 5;
        if (value.length() != 5 || "é".length() != 1 || "😀".length() != 2) return 6;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
