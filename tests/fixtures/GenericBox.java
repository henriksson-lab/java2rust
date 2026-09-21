public class GenericBox<T extends Comparable<T>> {
    private T value;
    public GenericBox(T value) { this.value = value; }
    public T get() { return value; }
    public boolean isBigger(T other) { return value.compareTo(other) > 0; }
    public static <U> U identity(U u) { return u; }
}
