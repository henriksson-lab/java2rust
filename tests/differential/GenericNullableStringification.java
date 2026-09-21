public class GenericNullableStringification {
    static class Payload {
        public String toString() { return "payload"; }
    }

    static class Box<T> {
        T value;
        Box(T value) { this.value = value; }
        String render() {
            StringBuilder builder = new StringBuilder();
            builder.append(value);
            return String.valueOf(value) + ":" + builder.toString();
        }
    }

    public static String run() {
        Box<String> text = new Box<>("x");
        Box<String> absent = new Box<>(null);
        Box<Integer> number = new Box<>(2);
        Box<Payload> source = new Box<>(new Payload());
        return text.render() + "," + absent.render() + ","
            + number.render() + "," + source.render();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
