class DirectArrayBase {
    DirectArrayBase() {}
    int first(int[] values) { return -4; }
}

public class AnonymousClassDirectArrayContract {
    static DirectArrayBase make() {
        return new DirectArrayBase() {
            @Override int first(int[] values) { return values[0]; }
        };
    }

    public static void main(String[] args) {
        DirectArrayBase value = make();
        System.out.println(value.first(new int[] {5}));
        try {
            value.first(new int[] {});
            System.out.println("unexpected");
        } catch (ArrayIndexOutOfBoundsException expected) {
            System.out.println("caught");
        }
    }
}
