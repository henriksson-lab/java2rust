import java.util.ArrayDeque;

public class QueuePollNull {
    public static String run() {
        ArrayDeque<String> queue = new ArrayDeque<String>();
        queue.add("a");
        String first = queue.poll();
        return first + ":" + queue.poll() + ":" + queue.isEmpty();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
