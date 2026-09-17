import java.util.concurrent.ArrayBlockingQueue;

public class ErasedArrayBlockingQueueLongDisplay {
    public static String run() {
        ArrayBlockingQueue<Long> queue = new ArrayBlockingQueue<Long>(3);
        queue.add(2L);
        queue.add(1L);
        Object value = queue;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
