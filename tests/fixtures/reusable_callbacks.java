import java.util.function.Supplier;
import java.util.function.Function;

class ReusableCallbacks {
    static <T> T consume(T value) { return value; }
    static <T> Supplier<T> expression(T value) { return () -> consume(value); }
    static <T> Supplier<T> block(T value) { return () -> { return consume(value); }; }
    static <T> Supplier<T> direct(T value) { return () -> value; }
    static <T> Supplier<Boolean> readOnly(T value) { return () -> value == null; }
    static <T> Function<T, T> fresh() { return value -> value; }

    static int run() {
        Supplier<String> first = expression("one");
        Supplier<String> second = block("two");
        Supplier<String> third = direct("three");
        return first.get().length() + first.get().length()
            + second.get().length() + second.get().length()
            + third.get().length() + third.get().length();
    }
}
