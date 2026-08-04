public class Shared {
    static class Node { int v; Node next; }
    public int run() {
        Node a = new Node();
        Node b = a;
        a.v = 5;
        return b.v;
    }
}
