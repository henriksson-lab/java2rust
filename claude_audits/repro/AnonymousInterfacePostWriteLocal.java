interface SnapshotCounter {
    int step(int delta);
    int read();
}

public class AnonymousInterfacePostWriteLocal {
    public static void main(String[] args) {
        SnapshotCounter counter = new SnapshotCounter() {
            int total = 1;

            public int step(int delta) {
                int next = this.total + delta;
                this.total = next;
                int snapshot = this.total;
                return snapshot;
            }

            public int read() {
                return this.total;
            }
        };
        counter.step(3);
        System.out.println(counter.step(2) * 10 + counter.read());
    }
}
