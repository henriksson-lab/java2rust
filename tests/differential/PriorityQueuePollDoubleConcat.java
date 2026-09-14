import java.util.PriorityQueue;

public class PriorityQueuePollDoubleConcat {
    public static String run() {
        PriorityQueue<Double> queue = new PriorityQueue<Double>();
        queue.add(1.0);
        queue.add(3.0);
        queue.add(2.0);
        return queue.poll() + "," + queue.poll() + "," + queue.poll() + "," + queue.poll();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
