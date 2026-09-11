import java.util.concurrent.ArrayBlockingQueue;

public class ArrayBlockingQueueOffer {
    public static String run() {
        ArrayBlockingQueue<String> queue = new ArrayBlockingQueue<String>(1);
        boolean first = queue.offer("a");
        boolean second = queue.offer("b");
        return first + "," + second + "," + queue.peek();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
