package client;
import api.Scope.*;
public class ValueReader<T> implements Reader<T> {
    public T read(T value) {
        if (value == null) throw new IllegalArgumentException("missing");
        return value;
    }
}
