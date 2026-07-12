package examples.objects;

interface Named {
    String name();

    default String display() {
        return "[" + name() + "]";
    }
}

public class InterfaceDefault implements Named {
    private final String value;

    public InterfaceDefault(String value) {
        this.value = value;
    }

    public String name() {
        return value;
    }
}

