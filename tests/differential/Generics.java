public class Generics {
    static class Box<T extends Comparable<T>> {
        private T value;
        Box(T value) { this.value = value; }
        T get() { return value; }
        boolean isBigger(T other) { return value.compareTo(other) > 0; }
    }
    static <U extends Comparable<U>> U max(U a, U b) { return a.compareTo(b) >= 0 ? a : b; }
    public static String run() {
        Box<Integer> box = new Box<>(5);
        return box.get() + "," + box.isBigger(3) + "," + box.isBigger(9) + "," + max(4, 11);
    }
    public static void main(String[] args) { System.out.println(run()); }
}
