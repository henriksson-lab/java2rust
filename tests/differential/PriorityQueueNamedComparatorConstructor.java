import java.util.Comparator;
import java.util.PriorityQueue;

public class PriorityQueueNamedComparatorConstructor {
    static class ReverseComparator implements Comparator<Integer> {
        ReverseComparator() {}

        public int compare(Integer left, Integer right) {
            return right.compareTo(left);
        }
    }

    public static String run() {
        PriorityQueue<Integer> queue = new PriorityQueue<Integer>(new ReverseComparator());
        queue.add(1);
        queue.add(3);
        queue.add(2);
        return queue.remove() + "," + queue.remove() + "," + queue.remove();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
