class TraceBox {
    int value;

    void add(int amount) {
        this.value = this.value + amount;
    }

    int read() {
        return this.value;
    }
}

interface Runner {
    int run(int delta);
    int current();
}

public class AnonymousInterfaceReadOnlyCall {
    public static void main(String[] args) {
        TraceBox sink = new TraceBox();
        Runner runner = new Runner() {
            int base = 2;

            public int run(int delta) {
                sink.add(this.base + delta);
                return sink.read();
            }

            public int current() {
                return sink.read();
            }
        };
        int first = runner.run(3);
        int second = runner.run(4);
        System.out.println(first * 100 + second * 10 + runner.current() + sink.read());
    }
}
