import java.util.concurrent.atomic.AtomicBoolean;

public class ErasedAtomicBooleanDisplay {
    public static String run() {
        Object value = new AtomicBoolean(false);
        value = new AtomicBoolean(((AtomicBoolean) value).getAndSet(true));
        return "x" + value;
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
