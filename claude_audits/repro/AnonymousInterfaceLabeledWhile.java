interface WhileCounter { int advance(); int read(); }

public class AnonymousInterfaceLabeledWhile {
    public static void main(String[] args) {
        WhileCounter counter = new WhileCounter() {
            int total = 0;

            public int advance() {
                int i = 0;
                outer: while (i < 5) {
                    i++;
                    int inner = 0;
                    while (inner < 2) {
                        inner++;
                        if (i == 2) continue outer;
                        this.total = this.total + i + inner;
                        if (i == 4) break outer;
                    }
                }
                return this.total * 10 + i;
            }

            public int read() { return this.total; }
        };
        System.out.println(counter.advance() * 100 + counter.read());
    }
}
