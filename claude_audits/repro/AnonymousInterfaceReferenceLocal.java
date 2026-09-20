interface ReferenceCounter { int compute(); int computeDirect(); int read(); }
class ReferenceNode {
    int value;
    ReferenceNode(int value) { this.value = value; }
    void set(int value) { this.value = value; }
    int read() { return this.value; }
}

public class AnonymousInterfaceReferenceLocal {
    static ReferenceCounter make(ReferenceNode original) {
        return new ReferenceCounter() {
            ReferenceNode node = original;
            int total = 0;

            public int compute() {
                ReferenceNode alias = this.node;
                alias.set(alias.read() + 1);
                this.total = alias.read();
                return this.total;
            }

            public int computeDirect() {
                if (this.total == 6) {
                    ReferenceNode alias = this.node;
                    alias.value = alias.value + 2;
                    this.total = alias.value;
                }
                return this.total;
            }

            public int read() { return this.node.value + this.total; }
        };
    }

    public static void main(String[] args) {
        ReferenceNode original = new ReferenceNode(3);
        ReferenceCounter counter = make(original);
        original.value = 5;
        System.out.println(counter.compute() * 1000 + counter.computeDirect() * 100
            + counter.read() * 10 + original.read());
    }
}
