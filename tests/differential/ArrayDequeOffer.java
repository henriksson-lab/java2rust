import java.util.ArrayDeque;

public class ArrayDequeOffer {
    public static String run() {
        ArrayDeque<String> q = new ArrayDeque<>();
        boolean a = q.offer("a");
        boolean b = q.offer("b");
        return a + "," + b + ":" + q.remove() + q.remove() + ":" + q.isEmpty();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
