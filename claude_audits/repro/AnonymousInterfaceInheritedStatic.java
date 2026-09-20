interface InheritedCounter {
    int add(int delta);
    int read();
}

class InheritedCounterBase {
    InheritedCounter counter = new InheritedCounter() {
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

class InheritedCounterChild extends InheritedCounterBase {}

public class AnonymousInterfaceInheritedStatic {
    static InheritedCounterChild holder = new InheritedCounterChild();

    public static void main(String[] args) {
        System.out.println(holder.run());
    }
}
