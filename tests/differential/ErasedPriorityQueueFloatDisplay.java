import java.util.PriorityQueue;

public class ErasedPriorityQueueFloatDisplay {
    public static String run() {
        PriorityQueue<Float> queue = new PriorityQueue<Float>();
        queue.add(2.0f);
        queue.add(1.0f);
        Object value = queue;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
