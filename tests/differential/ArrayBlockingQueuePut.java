import java.util.concurrent.ArrayBlockingQueue;

public class ArrayBlockingQueuePut {
    public static String run() {
        ArrayBlockingQueue<String> queue = new ArrayBlockingQueue<String>(2);
        try {
            queue.put("a");
            queue.put("b");
            return queue.peek() + "," + queue.poll() + "," + queue.poll() + "," + queue.poll();
        } catch (InterruptedException ex) {
            return "interrupted";
        }
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
