package api;
public class Scope {
    public interface Reader<T> {
        T read(T value);
    }
}
