class CheckedOverrideBase {
    CheckedOverrideBase() {}

    int select(int[] values) {
        return values[0];
    }
}

public class AnonymousClassFallibleBody {
    static CheckedOverrideBase make() {
        return new CheckedOverrideBase() {
            @Override int select(int[] values) {
                return values[1];
            }
        };
    }

    public static void main(String[] args) {
        CheckedOverrideBase value = make();
        System.out.println(value.select(new int[] {4, 7}));
        try {
            value.select(new int[] {});
            System.out.println("unexpected");
        } catch (ArrayIndexOutOfBoundsException expected) {
            System.out.println("caught");
        }
    }
}
