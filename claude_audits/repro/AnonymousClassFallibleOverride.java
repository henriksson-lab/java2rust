class CheckedArrayBase {
    CheckedArrayBase() {}

    int first(int[] values) {
        return values[0];
    }
}

public class AnonymousClassFallibleOverride {
    static CheckedArrayBase make() {
        return new CheckedArrayBase() {
            @Override int first(int[] values) {
                return values.length;
            }
        };
    }

    public static void main(String[] args) {
        int[] empty = {};
        CheckedArrayBase value = make();
        System.out.println(value.first(empty));
    }
}
