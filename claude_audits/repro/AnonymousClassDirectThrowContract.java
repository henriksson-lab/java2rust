class DirectThrowBase {
    DirectThrowBase() {}
    int read(boolean fail) { return -4; }
}

public class AnonymousClassDirectThrowContract {
    static DirectThrowBase make() {
        return new DirectThrowBase() {
            @Override int read(boolean fail) {
                if (fail) throw new IllegalStateException("boom");
                return 7;
            }
        };
    }

    public static void main(String[] args) {
        DirectThrowBase value = make();
        System.out.println(value.read(false));
        try {
            value.read(true);
            System.out.println("unexpected");
        } catch (IllegalStateException expected) {
            System.out.println("caught");
        }
    }
}
