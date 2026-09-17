public class SynchronizedMethodReentry {
    private int value;

    synchronized void increment() {
        value += 1;
        notifyAll();
    }

    synchronized void reenter() {
        increment();
    }

    public static String run() {
        SynchronizedMethodReentry value = new SynchronizedMethodReentry();
        value.reenter();
        return String.valueOf(value.value);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
