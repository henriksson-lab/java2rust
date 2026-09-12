import java.util.ArrayDeque;
import java.util.LinkedList;
import java.util.concurrent.ArrayBlockingQueue;

public class QueuePollAliases {
    public static String run() {
        ArrayDeque<String> deque = new ArrayDeque<String>();
        deque.add("a");
        String dequeFirst = deque.pollFirst();

        LinkedList<String> list = new LinkedList<String>();
        list.offer("b");
        String listFirst = list.poll();

        ArrayBlockingQueue<String> queue = new ArrayBlockingQueue<String>(2);
        queue.add("c");
        String queueFirst = queue.poll();

        return dequeFirst + ":" + deque.pollFirst()
            + ":" + listFirst + ":" + list.poll()
            + ":" + queueFirst + ":" + queue.poll();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
