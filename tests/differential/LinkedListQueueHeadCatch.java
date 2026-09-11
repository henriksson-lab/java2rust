import java.util.LinkedList;
import java.util.NoSuchElementException;

public class LinkedListQueueHeadCatch {
    public static String run() {
        LinkedList<String> q = new LinkedList<>();
        boolean offeredA = q.offer("a");
        boolean offeredB = q.offer("b");

        String first = q.element();
        String removed = q.remove();
        String next = q.getFirst();
        String popped = q.pop();

        try {
            q.removeFirst();
            return "miss";
        } catch (NoSuchElementException e) {
            return offeredA + "," + offeredB + ":" + first + removed + next + popped + ":caught";
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
