package examples.methods;

public class ConstructorOverloads {
    private final String label;
    private final int count;

    public ConstructorOverloads() {
        this("empty", 0);
    }

    public ConstructorOverloads(String label) {
        this(label, 1);
    }

    public ConstructorOverloads(String label, int count) {
        this.label = label;
        this.count = count;
    }

    public String describe() {
        return label + ":" + count;
    }
}

