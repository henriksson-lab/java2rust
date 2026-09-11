import java.util.ArrayDeque;
import java.util.NoSuchElementException;

public class ArrayDequeFirstPopCatch {
    public static String run() {
        ArrayDeque<String> q = new ArrayDeque<>();
        q.add("a");
        q.add("b");

        String first = q.getFirst();
        String popped = q.pop();
        String removed = q.removeFirst();

        try {
            q.getFirst();
            return "miss-get";
        } catch (NoSuchElementException e) {}

        try {
            q.pop();
            return "miss-pop";
        } catch (NoSuchElementException e) {}

        try {
            q.removeFirst();
            return "miss-remove";
        } catch (NoSuchElementException e) {}

        return first + popped + removed + ":caught";
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
