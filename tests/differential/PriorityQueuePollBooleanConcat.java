import java.util.PriorityQueue;

public class PriorityQueuePollBooleanConcat {
    public static String run() {
        PriorityQueue<Boolean> queue = new PriorityQueue<Boolean>();
        queue.add(true);
        queue.add(false);
        queue.add(true);
        return queue.poll() + "," + queue.poll() + "," + queue.poll() + "," + queue.poll();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
