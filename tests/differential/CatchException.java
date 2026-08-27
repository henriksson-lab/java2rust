public class CatchException {
    static String call(int n) {
        try {
            if (n == 0) throw new IllegalStateException("ise");
            return "ok";
        } catch (Exception e) {
            return "EX:" + e.getMessage();
        }
    }
    public static String run() { return call(0) + "," + call(1); }
    public static void main(String[] args) { System.out.println(run()); }
}
