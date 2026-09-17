public class AnonymousSupplier {
    interface IntSupplier {
        int get();
    }

    static int call(IntSupplier supplier) {
        return supplier.get();
    }

    public static String run() {
        return String.valueOf(call(new IntSupplier() {
            public int get() {
                return 5;
            }
        }));
    }

    public static void main(String[] args) {
        System.out.print(run());
    }
}
