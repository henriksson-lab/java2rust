/** Length uses the same Java unit at direct, callback and sequence boundaries. */
public class StringLengthProbe {
    interface Measure { int size(String text); }
    interface Size { int get(); }
    static boolean same(String left, String right) { return left == right; }
    static int count(String text) { return text.length(); }
    static int nullableCount(String text) {
        try { return text.length(); } catch (NullPointerException ex) { return -1; }
    }
    public static int run() {
        String text = new String("Aé😀Z");
        if (!same(text, text)) return 1;
        if (text.length() != 5 || count(text) != 5) return 2;
        Measure unbound = String::length;
        Size bound = text::length;
        if (unbound.size(text) != 5 || bound.get() != 5 || bound.get() != 5) return 3;
        if (nullableCount(null) != -1 || nullableCount(text) != 5) return 4;
        String[] texts = {"", "é", "😀", "Aé😀Z"};
        int sum = 0;
        for (String value : texts) sum += count(value);
        if (sum != 8) return 5;
        StringBuilder builder = new StringBuilder("é");
        CharSequence sequence = builder;
        if (builder.length() != 1 || sequence.length() != 1) return 6;
        builder.append("😀");
        if (builder.length() != 3 || sequence.length() != 3) return 7;
        StringBuffer buffer = new StringBuffer("😀");
        CharSequence buffered = buffer;
        if (buffer.length() != 2 || buffered.length() != 2) return 8;
        buffer.append("é");
        if (buffer.length() != 3 || buffered.length() != 3) return 9;
        if (new LengthLookalike().length() != 17) return 10;
        try { unbound.size(null); return 11; } catch (NullPointerException ex) { }
        try {
            String absent = null;
            Size invalid = absent::length;
            return 12;
        } catch (NullPointerException ex) { }
        return 0;
    }
    static class LengthLookalike { int length() { return 17; } }
    public static void main(String[] args) { System.out.println(run()); }
}
