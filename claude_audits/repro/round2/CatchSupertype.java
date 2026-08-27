public class CatchSupertype {
    static class MyError extends RuntimeException {
        MyError(String m) { super(m); }
    }
    static String call(int n) {
        try {
            if (n == 0) throw new MyError("mine");
            return "ok";
        } catch (RuntimeException e) {
            return "RTE:" + e.getMessage();
        }
    }
    public static String run() { return call(0) + "," + call(1); }
    public static void main(String[] args) { System.out.println(run()); }
}
