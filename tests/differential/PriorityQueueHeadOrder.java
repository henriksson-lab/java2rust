import java.util.PriorityQueue;

public class PriorityQueueHeadOrder {
    public static String run() {
        PriorityQueue<Integer> q = new PriorityQueue<Integer>();
        boolean a = q.add(5);
        boolean b = q.add(1);
        int first = q.element();
        int removed = q.remove();
        int next = q.element();
        return a + ":" + b + ":" + first + ":" + removed + ":" + next;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
