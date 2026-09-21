public class GenericErasedDisplay {
    static class Box<T> {
        private T value;
        Box(T value) { this.value = value; }
        public String toString() { return "Box"; }
    }

    public static String run() {
        Object value = new Box<String>("a");
        Box<Integer> diamond = new Box<>(2);
        Object second = diamond;
        return "x" + value + ":" + second;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
