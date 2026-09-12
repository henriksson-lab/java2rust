import java.util.concurrent.ArrayBlockingQueue;

public class ArrayBlockingQueueBadCapacity {
    public static String run() {
        try {
            new ArrayBlockingQueue<String>(0);
            return "made";
        } catch (IllegalArgumentException ex) {
            return ex.getClass().getSimpleName();
        }
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
