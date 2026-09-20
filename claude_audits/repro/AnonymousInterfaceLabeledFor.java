interface LabeledCounter { int advance(int limit); int read(); }

public class AnonymousInterfaceLabeledFor {
    public static void main(String[] args) {
        LabeledCounter counter = new LabeledCounter() {
            int total = 0;

            public int advance(int limit) {
                int i = 0;
                outer: for (i = 0; i < limit; i++) {
                    int inner = 0;
                    while (inner < 3) {
                        inner++;
                        if (inner == 2) continue outer;
                        this.total = this.total + i + inner;
                        if (i == 3) break outer;
                    }
                }
                return this.total + i;
            }

            public int read() { return this.total; }
        };
        System.out.println(counter.advance(5) * 100 + counter.read());
    }
}
