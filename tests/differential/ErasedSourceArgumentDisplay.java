public class ErasedSourceArgumentDisplay {
    static class Payload {
        private final int number;
        Payload(int number) { this.number = number; }
        public String toString() { return "P" + number; }
    }

    static String show(Object value) { return "" + value; }

    public static String run() {
        Object value = new Payload(7);
        return show(value);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
