interface LoopCounter {
    int step(int count);
    int read();
}

public class AnonymousInterfaceWhileWrites {
    public static void main(String[] args) {
        LoopCounter counter = new LoopCounter() {
            int total = 1;

            public int step(int count) {
                int index = 0;
                while (index < count && this.total < 100) {
                    this.total = this.total + index + 1;
                    index = index + 1;
                }
                return this.total;
            }

            public int read() {
                return this.total;
            }
        };
        int before = counter.step(3);
        int after = counter.step(2);
        System.out.println(before * 100 + after * 10 + counter.read());
    }
}
