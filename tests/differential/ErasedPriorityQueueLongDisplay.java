import java.util.PriorityQueue;

public class ErasedPriorityQueueLongDisplay {
    public static String run() {
        PriorityQueue<Long> queue = new PriorityQueue<Long>();
        queue.add(2L);
        queue.add(1L);
        Object value = queue;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
