import java.util.PriorityQueue;

public class PriorityQueuePollIntegerConcat {
    public static String run() {
        PriorityQueue<Integer> queue = new PriorityQueue<Integer>();
        queue.add(1);
        queue.add(3);
        queue.add(2);
        return queue.poll() + "," + queue.poll() + "," + queue.poll() + "," + queue.poll();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
