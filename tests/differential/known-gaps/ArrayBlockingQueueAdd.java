import java.util.concurrent.ArrayBlockingQueue;

public class ArrayBlockingQueueAdd {
    public static String run() {
        ArrayBlockingQueue<String> queue = new ArrayBlockingQueue<String>(1);
        boolean first = queue.add("a");
        try {
            queue.add("b");
            return first + ",no-throw";
        } catch (IllegalStateException ex) {
            return first + "," + queue.peek() + "," + ex.getClass().getSimpleName();
        }
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
