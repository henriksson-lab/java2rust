import java.util.Objects;

public class GenericObjectsToStringDefault {
    static int order = 0;

    static String valueFirst() { order = order * 10 + 1; return "v"; }
    static String defaultSecond() { order = order * 10 + 2; return "d"; }

    static class Payload {
        public String toString() { return "payload"; }
    }

    static class OrderedPayload {
        public String toString() { order = order * 10 + 3; return "ordered"; }
    }

    static OrderedPayload orderedValue() {
        order = order * 10 + 1;
        return new OrderedPayload();
    }

    static class Box<T> {
        T value;
        Box(T value) { this.value = value; }
        String render() { return Objects.toString(value, "fallback"); }
        boolean nullDefaultReturnsNull() {
            return Objects.toString(value, null) == null;
        }
        boolean nullableFallbackReturnsNull(String fallback) {
            return Objects.toString(value, fallback) == null;
        }
        String nullableResult() { return Objects.toString(value, null); }
    }

    public static String run() {
        String presentLocal = "z";
        Object erased = new Payload();
        Box<String> text = new Box<>("x");
        Box<String> absent = new Box<>(null);
        Box<Integer> number = new Box<>(2);
        Box<Payload> source = new Box<>(new Payload());
        return text.render() + "," + absent.render() + ","
            + number.render() + "," + source.render() + ","
            + text.nullDefaultReturnsNull() + ","
            + absent.nullDefaultReturnsNull() + ","
            + text.nullableFallbackReturnsNull(null) + ","
            + absent.nullableFallbackReturnsNull(null) + ","
            + absent.nullableFallbackReturnsNull("other") + ","
            + (text.nullableResult() == null) + ","
            + (absent.nullableResult() == null) + ","
            + (Objects.toString(presentLocal, null) == null) + ","
            + Objects.toString(valueFirst(), defaultSecond()) + ":" + order
            + "," + orderedDisplay() + ","
            + Objects.toString(erased, "fallback");
    }

    static String orderedDisplay() {
        order = 0;
        return Objects.toString(orderedValue(), defaultSecond()) + ":" + order;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
