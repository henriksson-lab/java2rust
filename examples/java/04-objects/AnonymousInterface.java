package examples.objects;

public class AnonymousInterface {
    interface IntPredicate {
        boolean test(int value);
    }

    public IntPredicate positive() {
        return new IntPredicate() {
            public boolean test(int value) {
                return value > 0;
            }
        };
    }
}

