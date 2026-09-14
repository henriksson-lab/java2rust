import java.util.concurrent.ArrayBlockingQueue;

public class ErasedArrayBlockingQueueDisplay {
    public static String run() {
        ArrayBlockingQueue<String> queue = new ArrayBlockingQueue<String>(3);
        queue.add("b");
        queue.add("a");
        Object value = queue;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
