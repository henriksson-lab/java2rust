import java.util.Comparator;
import java.util.PriorityQueue;

public class PriorityQueueComparatorFactoryLocalConstructor {
    static class ReverseComparator implements Comparator<Integer> {
        public int compare(Integer left, Integer right) {
            return right.compareTo(left);
        }
    }

    static Comparator<Integer> make() {
        return new ReverseComparator();
    }

    public static String run() {
        Comparator<Integer> comparator = make();
        PriorityQueue<Integer> queue = new PriorityQueue<Integer>(comparator);
        queue.add(1);
        queue.add(3);
        queue.add(2);
        return queue.remove() + "," + queue.remove() + "," + queue.remove();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
