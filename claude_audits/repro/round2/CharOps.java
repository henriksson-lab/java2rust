public class CharOps {
    public static String run() {
        char c = 'a';
        char up = (char) (c - 32);
        int code = c;
        char next = (char) (c + 1);
        String s = "hello";
        char at = s.charAt(1);
        int sum = 0;
        for (int i = 0; i < s.length(); i++) { sum += s.charAt(i); }
        String kind;
        switch (at) {
            case 'e': kind = "vowel"; break;
            case 'z': kind = "zed"; break;
            default: kind = "other";
        }
        boolean cmp = c < 'b';
        return "" + c + "," + up + "," + code + "," + next + "," + at + "," + sum + "," + kind + "," + cmp;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
