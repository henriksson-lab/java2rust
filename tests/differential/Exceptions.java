public class Exceptions {
    static int div(int a, int b) { return a / b; }
    static int guarded(int a, int b) {
        try { return div(a, b); } catch (ArithmeticException e) { return -1; }
    }
    static String finallyOrder() {
        StringBuilder sb = new StringBuilder();
        try { sb.append("t"); return sb.toString(); } finally { sb.append("f"); }
    }
    public static String run() {
        return div(7, 2) + "," + guarded(1, 0) + "," + guarded(9, 3) + "," + finallyOrder();
    }
    public static void main(String[] args) { System.out.println(run()); }
}
