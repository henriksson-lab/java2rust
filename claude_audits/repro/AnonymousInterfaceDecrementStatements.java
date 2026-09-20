interface Countdown {
    int tick();
    int read();
}

public class AnonymousInterfaceDecrementStatements {
    public static void main(String[] args) {
        Countdown counter = new Countdown() {
            int total = 5;

            public int tick() {
                this.total--;
                --this.total;
                return this.total;
            }

            public int read() {
                return this.total;
            }
        };
        int first = counter.tick();
        int second = counter.tick();
        System.out.println(first * 100 + second * 10 + counter.read());
    }
}
