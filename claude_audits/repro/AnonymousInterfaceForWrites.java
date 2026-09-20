interface ForCounter {
    int step(int count);
    int read();
}

public class AnonymousInterfaceForWrites {
    public static void main(String[] args) {
        ForCounter counter = new ForCounter() {
            int total = 1;

            public int step(int count) {
                for (int index = 0; index < count && this.total < 100; index++) {
                    if (index == 1) {
                        continue;
                    }
                    this.total = this.total + index + 1;
                    if (index == 3) {
                        break;
                    }
                }
                return this.total;
            }

            public int read() {
                return this.total;
            }
        };
        int before = counter.step(5);
        int after = counter.step(2);
        System.out.println(before * 100 + after * 10 + counter.read());
    }
}
