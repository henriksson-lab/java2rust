interface DoCounter { int advance(); int read(); }

public class AnonymousInterfaceLabeledDoWhile {
    public static void main(String[] args) {
        DoCounter counter = new DoCounter() {
            int total = 0;

            public int advance() {
                int i = 0;
                outer: do {
                    i++;
                    int inner = 0;
                    while (inner < 1) {
                        inner++;
                        if (i == 4) continue outer;
                        this.total = this.total + i + inner;
                    }
                } while (i < 4);
                return this.total * 10 + i;
            }

            public int read() { return this.total; }
        };
        System.out.println(counter.advance() * 100 + counter.read());
    }
}
