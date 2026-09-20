class AliasBase {
    AliasBase() {}
    int bump(int delta) { return delta; }
    int read() { return 0; }
}

public class AnonymousClassAlias {
    static AliasBase make() {
        return new AliasBase() {
            int count = 1;

            @Override int bump(int delta) {
                this.count = this.count + delta;
                return this.count;
            }

            @Override int read() { return this.count; }
        };
    }

    public static void main(String[] args) {
        AliasBase first = make();
        AliasBase second = first;
        first.bump(3);
        second.bump(2);
        System.out.println(first.read() + second.read());
    }
}
