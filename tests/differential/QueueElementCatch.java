import java.util.ArrayDeque;
import java.util.NoSuchElementException;

public class QueueElementCatch {
    public static String run() {
        ArrayDeque<String> q = new ArrayDeque<>();
        q.add("a");
        String first = q.element();
        String removed = q.remove();
        try {
            q.element();
            return "miss";
        } catch (NoSuchElementException e) {
            return first + removed + ":caught";
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
