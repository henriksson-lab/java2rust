public class SwitchChar {
    static String classify(char c) {
        String r = "";
        switch (c) {
            case 'a':
            case 'e': r = "vowel"; break;
            case 'b': r = "bee";
            case 'c': r = r + "see"; break;
            default: r = "other";
        }
        return r;
    }
    static String kind(String s) {
        switch (s) {
            case "one": return "1";
            case "two": return "2";
            default: return "?";
        }
    }
    public static String run() {
        return classify('a') + "," + classify('b') + "," + classify('c') + "," + classify('z') + "," + kind("two") + "," + kind("nine");
    }
    public static void main(String[] args) { System.out.println(run()); }
}
