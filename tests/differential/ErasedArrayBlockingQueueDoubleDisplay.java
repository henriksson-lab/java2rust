import java.util.concurrent.ArrayBlockingQueue;

public class ErasedArrayBlockingQueueDoubleDisplay {
    public static String run() {
        ArrayBlockingQueue<Double> queue = new ArrayBlockingQueue<Double>(3);
        queue.add(2.0);
        queue.add(1.5);
        Object value = queue;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
