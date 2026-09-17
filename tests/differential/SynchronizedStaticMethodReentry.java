public class SynchronizedStaticMethodReentry {
    private static int value;

    static synchronized void increment() {
        value += 1;
    }

    static synchronized void reenter() {
        increment();
    }

    public static String run() {
        reenter();
        return String.valueOf(value);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
