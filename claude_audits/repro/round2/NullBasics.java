public class NullBasics {
    static String find(int k) {
        if (k == 1) return "one";
        return null;
    }
    static String describe(String s) {
        if (s == null) return "none";
        return s;
    }
    public static String run() {
        String a = find(1);
        String b = find(2);
        String c = null;
        String concat = "v=" + b;
        boolean eq = (b == null);
        boolean ne = (a != null);
        return a + "," + describe(a) + "," + describe(b) + "," + concat + "," + eq + "," + ne + "," + (c == null);
    }
    public static void main(String[] args) { System.out.println(run()); }
}
