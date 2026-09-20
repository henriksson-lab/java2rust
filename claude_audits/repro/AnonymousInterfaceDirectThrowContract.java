interface ThrowingCounter { int step(boolean fail); int read(); }

public class AnonymousInterfaceDirectThrowContract {
    static ThrowingCounter make() {
        return new ThrowingCounter() {
            int total = 0;

            public int step(boolean fail) {
                if (fail) throw new IllegalStateException("boom");
                this.total = this.total + 1;
                return this.total;
            }

            public int read() { return this.total; }
        };
    }

    public static void main(String[] args) {
        ThrowingCounter counter = make();
        System.out.println(counter.step(false) + counter.read());
        try {
            counter.step(true);
            System.out.println("unexpected");
        } catch (IllegalStateException expected) {
            System.out.println("caught");
        }
    }
}
