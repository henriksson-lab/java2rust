public class NullDeref {
    public static String run() {
        String s = null;
        try {
            int n = s.length();
            return "len:" + n;
        } catch (NullPointerException e) {
            return "npe";
        }
    }
    public static void main(String[] args) { System.out.println(run()); }
}
