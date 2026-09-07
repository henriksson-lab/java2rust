import java.util.concurrent.atomic.AtomicLong;

public class ErasedAtomicLongDisplay {
    public static String run() {
        Object value = new AtomicLong(3L);
        value = new AtomicLong(((AtomicLong) value).incrementAndGet());
        return "x" + value + ",direct=" + value.toString();
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
