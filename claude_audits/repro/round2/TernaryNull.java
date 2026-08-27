public class TernaryNull {
    static String pick(int n) { return n > 0 ? "pos" : null; }
    public static String run() {
        String a = pick(1);
        String b = pick(-1);
        String r = (b == null) ? "was-null" : b;
        int z = 0;
        int q = (z != 0) ? 10 / z : -1;
        return a + "," + r + "," + q;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
