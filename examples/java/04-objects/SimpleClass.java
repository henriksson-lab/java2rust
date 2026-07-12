package examples.objects;

public class SimpleClass {
    private final String name;
    private int count;

    public SimpleClass(String name) {
        this.name = name;
        this.count = 0;
    }

    public void increment() {
        this.count++;
    }

    public String label() {
        return this.name + ":" + this.count;
    }
}

