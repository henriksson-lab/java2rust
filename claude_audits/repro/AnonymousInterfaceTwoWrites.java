interface PairCounter {
    int step(int delta);
    int read();
}

public class AnonymousInterfaceTwoWrites {
    public static void main(String[] args) {
        PairCounter counter = new PairCounter() {
            int first = 1;
            int second = 2;

            public int step(int delta) {
                int next = this.first + delta;
                this.first = next;
                int later = this.second + next;
                this.second = later;
                int snapshot = this.first + this.second;
                return snapshot;
            }

            public int read() {
                return this.first + this.second;
            }
        };
        counter.step(3);
        System.out.println(counter.step(2) * 100 + counter.read());
    }
}
