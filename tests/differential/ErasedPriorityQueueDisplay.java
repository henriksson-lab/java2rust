import java.util.PriorityQueue;

public class ErasedPriorityQueueDisplay {
    public static String run() {
        PriorityQueue<Integer> queue = new PriorityQueue<Integer>();
        queue.add(2);
        queue.add(1);
        Object value = queue;
        return "value=" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
