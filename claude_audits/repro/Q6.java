public class Q6 {
    private int n = 0;
    public synchronized void inc() { n++; }
    public int run() { synchronized(this) { n = 5; } return n; }
}
