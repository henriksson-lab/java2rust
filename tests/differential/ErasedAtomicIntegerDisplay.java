import java.util.concurrent.atomic.AtomicInteger;

public class ErasedAtomicIntegerDisplay {
    public static String run() {
        Object value = new AtomicInteger(3);
        value = new AtomicInteger(((AtomicInteger) value).incrementAndGet());
        return "x" + value;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
