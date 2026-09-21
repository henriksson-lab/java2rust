interface AnonymousPrivateHelperValue {
    int get();
}

public class AnonymousPrivateHelper {
    static AnonymousPrivateHelperValue make(int seed) {
        return new AnonymousPrivateHelperValue() {
            private int adjusted(int value) {
                return value + 2;
            }

            public int get() {
                return this.adjusted(seed);
            }
        };
    }

    public static String run() {
        return String.valueOf(make(5).get());
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
