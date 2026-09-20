interface Ticker {
    int tick();
    int read();
}

public class AnonymousInterfaceIncrementStatements {
    public static void main(String[] args) {
        Ticker ticker = new Ticker() {
            int total = 1;

            public int tick() {
                this.total++;
                ++this.total;
                return this.total;
            }

            public int read() {
                return this.total;
            }
        };
        int first = ticker.tick();
        int second = ticker.tick();
        System.out.println(first * 100 + second * 10 + ticker.read());
    }
}
