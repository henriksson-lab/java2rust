import java.util.concurrent.ArrayBlockingQueue;

public class ErasedArrayBlockingQueueFloatDisplay {
    public static String run() {
        ArrayBlockingQueue<Float> queue = new ArrayBlockingQueue<Float>(3);
        queue.add(2.0f);
        queue.add(1.5f);
        Object value = queue;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
