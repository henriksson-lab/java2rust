class StaticBase {
    StaticBase() {}
    int read() { return 0; }
    int bump(int delta) { return delta; }
}

public class AnonymousClassStatic {
    static StaticBase cached = new StaticBase() {
        int count = 1;

        @Override int read() { return this.count; }

        @Override int bump(int delta) {
            this.count = this.count + delta;
            return this.count;
        }
    };

    public static void main(String[] args) {
        cached.bump(3);
        System.out.println(cached.read());
    }
}
