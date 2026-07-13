package examples.stdlib;

public class SynchronizedBlock {
    private int count;

    public void bump(Object lock) {
        synchronized (lock) {
            count++;
        }
    }
}
