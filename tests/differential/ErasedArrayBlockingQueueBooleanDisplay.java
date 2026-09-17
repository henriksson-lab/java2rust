import java.util.concurrent.ArrayBlockingQueue;

public class ErasedArrayBlockingQueueBooleanDisplay {
    public static String run() {
        ArrayBlockingQueue<Boolean> queue = new ArrayBlockingQueue<Boolean>(3);
        queue.add(true);
        queue.add(false);
        Object value = queue;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
