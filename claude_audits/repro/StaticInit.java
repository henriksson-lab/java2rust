public class StaticInit {
    static int counter = 0;
    static { counter = 5; }
    static int derived = counter * 2;
    public int run() { return derived; }
}
