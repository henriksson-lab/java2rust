class DeepSuperBase {
    DeepSuperBase() {}
    int checked(int[] values) { return values[0] + 2; }
    int adjusted(int[] values) { return -1; }
    int negated(int[] values) { return -1; }
}

public class AnonymousClassNestedSuperExpression {
    static DeepSuperBase make() {
        return new DeepSuperBase() {
            @Override int adjusted(int[] values) {
                return 2 * (1 + super.checked(values));
            }

            @Override int negated(int[] values) {
                return -(1 + super.checked(values));
            }
        };
    }

    public static void main(String[] args) {
        DeepSuperBase value = make();
        System.out.println(value.adjusted(new int[] {5}));
        System.out.println(value.negated(new int[] {5}));
        try {
            value.adjusted(new int[] {});
            System.out.println("unexpected");
        } catch (ArrayIndexOutOfBoundsException expected) {
            System.out.println("caught");
        }
        try {
            value.negated(new int[] {});
            System.out.println("unexpected");
        } catch (ArrayIndexOutOfBoundsException expected) {
            System.out.println("caught");
        }
    }
}
