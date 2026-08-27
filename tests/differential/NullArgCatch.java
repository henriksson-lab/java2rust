public class NullArgCatch {
    static int len(String s) {
        return s.length();
    }
    public static String run() {
        try {
            return "ok:" + len(null);
        } catch (NullPointerException e) {
            return "npe";
        }
    }
    public static void main(String[] args) { System.out.println(run()); }
}
