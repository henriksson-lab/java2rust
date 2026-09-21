public class GenericNullableFieldDisplay {
    static class Payload {
        public String toString() { return "payload"; }
    }

    static class Box<T> {
        T value;
        Box(T value) { this.value = value; }
        public String toString() { return "Box(" + value + ")"; }
    }

    public static String run() {
        Box<String> text = new Box<>("a");
        Box<String> absent = new Box<>(null);
        Box<Integer> number = new Box<>(2);
        Box<Payload> source = new Box<>(new Payload());
        return text + "," + absent + "," + number + "," + source;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
