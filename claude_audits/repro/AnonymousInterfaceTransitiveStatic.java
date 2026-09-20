interface EmbeddedCounter {
    int add(int delta);
    int read();
}

class CounterHolder {
    EmbeddedCounter counter = new EmbeddedCounter() {
        int total = 1;

        public int add(int delta) {
            this.total = this.total + delta;
            return this.total;
        }

        public int read() { return this.total; }
    };

    int run() {
        counter.add(3);
        return counter.read();
    }
}

public class AnonymousInterfaceTransitiveStatic {
    static CounterHolder holder = new CounterHolder();

    public static void main(String[] args) {
        System.out.println(holder.run());
    }
}
