interface Greeter {
    String name();
    default String greet() { return "hello " + name(); }
}
public class Q5 implements Greeter {
    public String name() { return "world"; }
    public String run() { return greet(); }
}
