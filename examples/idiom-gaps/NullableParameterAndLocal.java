package idiom.gaps;

public class NullableParameterAndLocal {
    public Object keepOrDefault(Object value) {
        Object current = null;
        if (value == null) {
            return current;
        }
        return value;
    }
}
