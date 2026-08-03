public class GenericBounds {
    static class Box<T extends Comparable<T>> {
        private T value;
        Box(T value) { this.value = value; }
        boolean isBigger(T other) { return value.compareTo(other) > 0; }
    }
    static <U extends Comparable<U>> boolean firstWins(U a, U b) { return a.compareTo(b) >= 0; }
    public static String run() {
        Box<Integer> box = new Box<>(5);
        return box.isBigger(3) + "," + box.isBigger(9) + "," + firstWins(11, 4) + "," + firstWins(2, 8);
    }
    public static void main(String[] args) { System.out.println(run()); }
}
