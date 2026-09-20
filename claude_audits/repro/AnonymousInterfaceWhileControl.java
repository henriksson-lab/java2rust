interface LoopControlCounter {
    int step(int count);
    int read();
}

public class AnonymousInterfaceWhileControl {
    public static void main(String[] args) {
        LoopControlCounter counter = new LoopControlCounter() {
            int total = 1;

            public int step(int count) {
                int index = 0;
                while (index < count) {
                    index = index + 1;
                    if (index == 2) {
                        continue;
                    }
                    this.total = this.total + index;
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
        int after = counter.step(1);
        System.out.println(before * 100 + after * 10 + counter.read());
    }
}
