class CheckedSuperBase {
    CheckedSuperBase() {}

    int read(int[] values) {
        return values[0] + 2;
    }

    int readPlus(int[] values) {
        return values[0] + 2;
    }
}

public class AnonymousClassFallibleSuper {
    static CheckedSuperBase make() {
        return new CheckedSuperBase() {
            @Override int read(int[] values) {
                return super.read(values);
            }

            @Override int readPlus(int[] values) {
                int base = super.readPlus(values);
                return base + 1;
            }
        };
    }

    public static void main(String[] args) {
        CheckedSuperBase value = make();
        System.out.println(value.read(new int[] {5}));
        System.out.println(value.readPlus(new int[] {5}));
        try {
            value.read(new int[] {});
            System.out.println("unexpected");
        } catch (ArrayIndexOutOfBoundsException expected) {
            System.out.println("caught");
        }
        try {
            value.readPlus(new int[] {});
            System.out.println("unexpected");
        } catch (ArrayIndexOutOfBoundsException expected) {
            System.out.println("caught");
        }
    }
}
