class OtherSuperBase {
    OtherSuperBase() {}

    int base(int value) { return value + 2; }
    int base(String ignored) { return -100; }
    int direct(int value) { return -1; }
    int viaLocal(int value) { return -2; }
    int inExpr(int value) { return -3; }
    int inExprRight(int value) { return -5; }
    int checked(int[] values) { return values[0] + 2; }
    int checked(String ignored) { return -6; }
    int checkedViaLocal(int[] values) { return values[0]; }
    int checkedExpr(int[] values) { return values[0]; }
    int checkedExprRight(int[] values) { return values[0]; }
    int checkedByOverride(int[] values) { return -4; }
}

public class AnonymousClassOtherSuperTarget {
    static OtherSuperBase make() {
        return new OtherSuperBase() {
            @Override int direct(int value) { return super.base(value); }

            @Override int viaLocal(int value) {
                int fromBase = super.base(value);
                return fromBase * 2;
            }

            @Override int inExpr(int value) {
                return super.base(value + 1) + 1;
            }

            @Override int inExprRight(int value) {
                return 1 + super.base(value + 1);
            }

            @Override int checkedViaLocal(int[] values) {
                int fromBase = super.checked(values);
                return fromBase + 1;
            }

            @Override int checkedExpr(int[] values) {
                return super.checked(values) + 1;
            }

            @Override int checkedExprRight(int[] values) {
                return 1 + super.checked(values);
            }

            @Override int checkedByOverride(int[] values) {
                return super.checked(values) + 1;
            }
        };
    }

    public static void main(String[] args) {
        OtherSuperBase value = make();
        System.out.println(value.direct(5) + value.viaLocal(5));
        System.out.println(value.inExpr(5));
        System.out.println(value.inExprRight(5));
        System.out.println(value.checkedViaLocal(new int[] {5}));
        System.out.println(value.checkedExpr(new int[] {5}));
        System.out.println(value.checkedExprRight(new int[] {5}));
        System.out.println(value.checkedByOverride(new int[] {5}));
        try {
            value.checkedViaLocal(new int[] {});
            System.out.println("unexpected");
        } catch (ArrayIndexOutOfBoundsException expected) {
            System.out.println("caught");
        }
        try {
            value.checkedByOverride(new int[] {});
            System.out.println("unexpected");
        } catch (ArrayIndexOutOfBoundsException expected) {
            System.out.println("caught");
        }
        try {
            value.checkedExprRight(new int[] {});
            System.out.println("unexpected");
        } catch (ArrayIndexOutOfBoundsException expected) {
            System.out.println("caught");
        }
        try {
            value.checkedExpr(new int[] {});
            System.out.println("unexpected");
        } catch (ArrayIndexOutOfBoundsException expected) {
            System.out.println("caught");
        }
    }
}
