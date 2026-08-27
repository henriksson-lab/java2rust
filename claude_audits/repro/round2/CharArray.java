public class CharArray {
    public static String run() {
        String s = "abc";
        char[] cs = s.toCharArray();
        StringBuilder sb = new StringBuilder();
        for (char c : cs) { sb.append((char)(c + 1)); }
        char x = 'a';
        x += 2;
        x++;
        String vs = String.valueOf(x);
        return sb.toString() + "," + cs.length + "," + x + "," + vs;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
