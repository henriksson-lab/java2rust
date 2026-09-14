import java.util.PriorityQueue;

public class PriorityQueueLambdaComparatorConstructor {
    public static String run() {
        PriorityQueue<Integer> queue = new PriorityQueue<Integer>((left, right) -> right - left);
        queue.add(1);
        queue.add(3);
        queue.add(2);
        return queue.remove() + "," + queue.remove() + "," + queue.remove();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
