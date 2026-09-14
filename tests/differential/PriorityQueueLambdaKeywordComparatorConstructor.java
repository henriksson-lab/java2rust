import java.util.PriorityQueue;

public class PriorityQueueLambdaKeywordComparatorConstructor {
    public static String run() {
        PriorityQueue<Integer> queue = new PriorityQueue<Integer>((type, value) -> value - type);
        queue.add(1);
        queue.add(3);
        queue.add(2);
        return queue.remove() + "," + queue.remove() + "," + queue.remove();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
