public class ThreadSleepCatch {
    public static String run() {
        try {
            Thread.sleep(-1);
            return "miss";
        } catch (IllegalArgumentException e) {
            return "iae";
        } catch (InterruptedException e) {
            return "interrupted";
        }
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
