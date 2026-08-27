public class NullConcat {
    static String maybe(boolean b) { return b ? "hi" : null; }
    public static String run() {
        String n = null;
        Object o = null;
        String s1 = "a" + n;
        String s2 = "b" + maybe(false);
        String s3 = "c" + o;
        return s1 + "," + s2 + "," + s3 + "," + (n == null);
    }
    public static void main(String[] args) { System.out.println(run()); }
}
