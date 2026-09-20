interface DoCounter {
    int step(int count);
    int read();
}

public class AnonymousInterfaceDoWhileWrites {
    public static void main(String[] args) {
        DoCounter counter = new DoCounter() {
            int total = 1;

            public int step(int count) {
                int index = 0;
                do {
                    index = index + 1;
                    if (index == 2) {
                        continue;
                    }
                    this.total = this.total + index;
                } while (index < count && this.total < 100);
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
