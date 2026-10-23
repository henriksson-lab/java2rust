package searchoffsets;

public class StringSearchOffsetProbe {
    static int forward(String text, char needle, int offset) {
        return text.indexOf(needle, offset + 1);
    }
    static int backward(String text, char needle, int offset) {
        return text.lastIndexOf(needle, offset - 1);
    }
    static int run() {
        String text = "ababa";
        if (forward(text, 'a', -9) != 0) return 1;
        if (forward(text, 'a', 0) != 2) return 2;
        if (forward(text, 'a', 4) != -1) return 3;
        if (forward(text, 'a', Integer.MAX_VALUE) != 0) return 4;
        if (backward(text, 'a', 0) != -1) return 5;
        if (backward(text, 'a', 4) != 2) return 6;
        if (backward(text, 'a', Integer.MIN_VALUE) != 4) return 7;
        if (text.indexOf("ba", -4 + 1) != 1) return 8;
        if (text.lastIndexOf("ba", 100 - 1) != 3) return 9;
        if ("".indexOf("", -3 + 1) != 0) return 10;
        if ("".lastIndexOf("", -3 - 1) != -1) return 11;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
