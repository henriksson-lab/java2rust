public class R3 {
    static int shared = 0;
    public void bump() { shared++; }
    public int get() { return shared; }
}
