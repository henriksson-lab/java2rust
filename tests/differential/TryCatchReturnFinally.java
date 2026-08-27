public class TryCatchReturnFinally {
    static int fail() { throw new RuntimeException("x"); }
    static String f(int n) {
        StringBuilder b = new StringBuilder();
        try {
            if (n == 0) fail();
            b.append("try");
            return b.toString();
        } catch (RuntimeException e) {
            b.append("catch");
            return b.toString();
        } finally {
            b.append("finally");
        }
    }
    public static String run() { return f(0) + "," + f(1); }
    public static void main(String[] args) { System.out.println(run()); }
}
