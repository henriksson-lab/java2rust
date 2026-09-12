import java.util.concurrent.ArrayBlockingQueue;

public class ArrayBlockingQueueNull {
    public static String run() {
        ArrayBlockingQueue<String> addQueue = new ArrayBlockingQueue<String>(1);
        ArrayBlockingQueue<String> offerQueue = new ArrayBlockingQueue<String>(1);
        String left = "";
        String right = "";
        try {
            addQueue.add(null);
            left = "add-ok";
        } catch (NullPointerException ex) {
            left = ex.getClass().getSimpleName();
        }
        try {
            offerQueue.offer(null);
            right = "offer-ok";
        } catch (NullPointerException ex) {
            right = ex.getClass().getSimpleName();
        }
        return left + "," + right;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
