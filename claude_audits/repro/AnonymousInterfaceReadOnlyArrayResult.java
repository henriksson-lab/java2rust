interface FallibleReadCounter {
    int read(int[] values);
    int state();
}

class FallibleReadCounterUse {
    static FallibleReadCounter make() {
        return new FallibleReadCounter() {
            int total = 7;

            public int read(int[] values) {
                return values[0] + this.total;
            }

            public int state() {
                return this.total;
            }
        };
    }

    static int run() {
        FallibleReadCounter counter = make();
        int[] values = {3};
        return counter.read(values) + counter.state();
    }

    static int runFail() {
        FallibleReadCounter counter = make();
        int[] values = {};
        return counter.read(values);
    }
}

public class AnonymousInterfaceReadOnlyArrayResult {
    public static void main(String[] args) {
        System.out.println(FallibleReadCounterUse.run());
        try {
            System.out.println(FallibleReadCounterUse.runFail());
        } catch (ArrayIndexOutOfBoundsException expected) {
            System.out.println("caught");
        }
    }
}
