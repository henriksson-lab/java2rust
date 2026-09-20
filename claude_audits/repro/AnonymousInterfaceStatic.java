interface StaticCounter {
    int add(int delta);
    int read();
    default int doubled() { return read() * 2; }
}

interface StaticAdder { int add(int value); }

class StaticAnonymousAdder {
    static StaticAdder adder = new StaticAdder() {
        public int add(int value) { return value + 1; }
    };

    static int run() { return adder.add(4); }
}

public class AnonymousInterfaceStatic {
    static StaticCounter counter = new StaticCounter() {
        int total = 1;

        public int add(int delta) {
            this.total = this.total + delta;
            return this.total;
        }

        public int read() { return this.total; }
    };

    public static void main(String[] args) {
        counter.add(3);
        System.out.println(counter.read());
        System.out.println(counter.doubled());
        System.out.println(StaticAnonymousAdder.run());
    }
}
