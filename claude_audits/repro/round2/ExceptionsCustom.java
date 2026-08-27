public class ExceptionsCustom {
    static class MyError extends RuntimeException {
        MyError(String m) { super(m); }
    }
    static int risky(int n) {
        if (n < 0) throw new MyError("neg:" + n);
        return n * 2;
    }
    static String call(int n) {
        try {
            return "ok:" + risky(n);
        } catch (MyError e) {
            return "caught:" + e.getMessage();
        }
    }
    public static String run() {
        return call(3) + "," + call(-1);
    }
    public static void main(String[] args) { System.out.println(run()); }
}
