package examples.types;

public class NullableLocalReturn {
    public NullableLocalReturn keepOrDefault(boolean useDefault, NullableLocalReturn value) {
        NullableLocalReturn current = null;
        if (useDefault) {
            return current;
        }
        return value;
    }
}
