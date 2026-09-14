import java.util.PriorityQueue;

public class PriorityQueueBlockLambdaComparatorConstructor {
    public static String run() {
        PriorityQueue<Integer> queue = new PriorityQueue<Integer>((left, right) -> {
            if (left < right) return 1;
            if (left > right) return -1;
            return 0;
        });
        queue.add(1);
        queue.add(3);
        queue.add(2);
        return queue.remove() + "," + queue.remove() + "," + queue.remove();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
