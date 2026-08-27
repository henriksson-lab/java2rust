public class ShortCircuitLocal {
    static int calls = 0;
    static boolean t(boolean v) { calls++; return v; }
    public static String run() {
        boolean r1 = t(false) && t(true);
        int c1 = calls;
        boolean r2 = t(true) || t(true);
        int c2 = calls;
        String s = null;
        boolean r4 = (s != null && s.length() > 0);
        return r1 + "," + c1 + "," + r2 + "," + c2 + "," + r4;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
