import java.util.ArrayDeque;
import java.util.NoSuchElementException;

public class QueueRemoveCatch {
    public static String run() {
        ArrayDeque<String> q = new ArrayDeque<>();
        q.add("a");
        String first = q.remove();
        try {
            q.remove();
            return "miss";
        } catch (NoSuchElementException e) {
            return first + ":caught";
        }
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
